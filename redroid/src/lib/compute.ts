import { InstancesClient, ZoneOperationsClient } from '@google-cloud/compute';
import { v4 as uuidv4 } from 'uuid';
import { VMConfig } from './types';
import { generateStartupScript } from './startup-script';

// Parse GCP credentials from environment variable
function getGCPCredentials() {
  const credentialsJson = process.env.GCP_CREDENTIALS;
  if (!credentialsJson) {
    throw new Error('GCP_CREDENTIALS environment variable is not set');
  }
  try {
    return JSON.parse(credentialsJson);
  } catch {
    throw new Error('GCP_CREDENTIALS is not valid JSON');
  }
}

// Get configuration from environment
export function getVMConfig(): VMConfig {
  return {
    projectId: process.env.GCP_PROJECT_ID || 'corsali-development',
    zone: process.env.GCP_ZONE || 'us-central1-a',
    machineType: process.env.VM_MACHINE_TYPE || 'e2-standard-4',
    diskSizeGb: parseInt(process.env.VM_DISK_SIZE_GB || '100', 10),
    ttlMinutes: parseInt(process.env.VM_TTL_MINUTES || '60', 10),
  };
}

// Create clients with credentials
function getInstancesClient() {
  const credentials = getGCPCredentials();
  return new InstancesClient({ credentials, projectId: credentials.project_id });
}

function getOperationsClient() {
  const credentials = getGCPCredentials();
  return new ZoneOperationsClient({ credentials, projectId: credentials.project_id });
}

export async function createVM(sessionId: string): Promise<string> {
  console.log('[COMPUTE] createVM called with sessionId:', sessionId);
  const config = getVMConfig();
  const instancesClient = getInstancesClient();
  
  const vmName = `redroid-${sessionId.slice(0, 8)}`;
  console.log('[COMPUTE] VM name:', vmName);
  const startupScript = generateStartupScript(config.ttlMinutes);
  console.log('[COMPUTE] Startup script length:', startupScript.length);

  console.log('[COMPUTE] Calling instancesClient.insert...');
  const [operation] = await instancesClient.insert({
    project: config.projectId,
    zone: config.zone,
    instanceResource: {
      name: vmName,
      machineType: `zones/${config.zone}/machineTypes/${config.machineType}`,
      tags: {
        items: ['redroid', 'allow-kasm', 'allow-adb'],
      },
      disks: [
        {
          boot: true,
          autoDelete: true,
          initializeParams: {
            sourceImage: 'projects/ubuntu-os-cloud/global/images/family/ubuntu-2204-lts',
            diskSizeGb: config.diskSizeGb.toString(),
            diskType: `zones/${config.zone}/diskTypes/pd-ssd`,
          },
        },
      ],
      networkInterfaces: [
        {
          network: 'global/networks/default',
          accessConfigs: [
            {
              name: 'External NAT',
              type: 'ONE_TO_ONE_NAT',
            },
          ],
        },
      ],
      metadata: {
        items: [
          {
            key: 'startup-script',
            value: startupScript,
          },
        ],
      },
      serviceAccounts: [
        {
          email: 'default',
          scopes: ['https://www.googleapis.com/auth/cloud-platform'],
        },
      ],
    },
  });

  console.log('[COMPUTE] Insert operation started:', operation.name);
  
  // Wait for the operation to complete
  if (operation.name) {
    console.log('[COMPUTE] Waiting for operation to complete...');
    await waitForOperation(operation.name, config.zone);
    console.log('[COMPUTE] Operation completed successfully');
  }

  return vmName;
}

export async function getVMStatus(vmName: string): Promise<{
  status: string;
  ip: string | null;
}> {
  const config = getVMConfig();
  const instancesClient = getInstancesClient();

  try {
    const [instance] = await instancesClient.get({
      project: config.projectId,
      zone: config.zone,
      instance: vmName,
    });

    const networkInterface = instance.networkInterfaces?.[0];
    const accessConfig = networkInterface?.accessConfigs?.[0];
    const ip = accessConfig?.natIP || null;

    return {
      status: instance.status || 'UNKNOWN',
      ip,
    };
  } catch (error: unknown) {
    const err = error as { code?: number };
    if (err.code === 404 || (err.code && err.code === 5)) {
      return { status: 'DELETED', ip: null };
    }
    throw error;
  }
}

export async function deleteVM(vmName: string): Promise<void> {
  const config = getVMConfig();
  const instancesClient = getInstancesClient();

  try {
    const [operation] = await instancesClient.delete({
      project: config.projectId,
      zone: config.zone,
      instance: vmName,
    });

    if (operation.name) {
      await waitForOperation(operation.name, config.zone);
    }
  } catch (error: unknown) {
    const err = error as { code?: number };
    // Ignore if VM is already deleted
    if (err.code !== 404 && err.code !== 5) {
      throw error;
    }
  }
}

async function waitForOperation(operationName: string, zone: string): Promise<void> {
  const config = getVMConfig();
  const operationsClient = getOperationsClient();

  let operation;
  do {
    await new Promise((resolve) => setTimeout(resolve, 2000));
    [operation] = await operationsClient.get({
      project: config.projectId,
      zone,
      operation: operationName,
    });
  } while (operation.status !== 'DONE');

  if (operation.error) {
    throw new Error(`Operation failed: ${JSON.stringify(operation.error)}`);
  }
}

// Check if noVNC is ready by attempting to connect to /vnc.html
export async function checkKasmReady(ip: string): Promise<boolean> {
  try {
    // Use AbortController for timeout
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 5000);
    
    // Check /vnc.html specifically - that's the noVNC entry point
    const response = await fetch(`https://${ip}/vnc.html`, {
      method: 'HEAD',
      signal: controller.signal,
      // @ts-expect-error - Node.js fetch option for self-signed certs
      rejectUnauthorized: false,
    });
    
    clearTimeout(timeoutId);
    console.log('[COMPUTE] checkKasmReady response status:', response.status);
    return response.ok || response.status === 401 || response.status === 302;
  } catch (error) {
    console.log('[COMPUTE] checkKasmReady error:', error);
    return false;
  }
}

// List all redroid-* VMs from GCP (source of truth for active sessions)
export async function listRedroidVMs(): Promise<Array<{
  name: string;
  status: string;
  ip: string | null;
  createdAt: string | null;
  zone: string;
}>> {
  const config = getVMConfig();
  const instancesClient = getInstancesClient();
  
  try {
    const [instances] = await instancesClient.list({
      project: config.projectId,
      zone: config.zone,
      filter: 'name:redroid-*',
    });
    
    return (instances || []).map(instance => ({
      name: instance.name || '',
      status: instance.status || 'UNKNOWN',
      ip: instance.networkInterfaces?.[0]?.accessConfigs?.[0]?.natIP || null,
      createdAt: instance.creationTimestamp || null,
      zone: config.zone,
    }));
  } catch (error) {
    console.error('[COMPUTE] Error listing VMs:', error);
    return [];
  }
}

export function generateSessionId(): string {
  return uuidv4();
}
