import { InstancesClient, ZoneOperationsClient } from '@google-cloud/compute';
import { v4 as uuidv4 } from 'uuid';
import https from 'https';
import http from 'http';
import { VMConfig, VMProgress } from './types';
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

// Check if ws-scrcpy is ready by attempting to connect to the root
export async function checkKasmReady(ip: string): Promise<boolean> {
  return new Promise((resolve) => {
    const timeout = setTimeout(() => {
      resolve(false);
    }, 5000);

    const req = https.request(
      {
        hostname: ip,
        port: 443,
        path: '/',
        method: 'HEAD',
        rejectUnauthorized: false, // Accept self-signed certs
        timeout: 5000,
      },
      (res) => {
        clearTimeout(timeout);
        console.log('[COMPUTE] checkKasmReady response status:', res.statusCode);
        // Accept any successful response (2xx, 3xx redirects, or 401 auth required)
        const status = res.statusCode || 0;
        resolve(status >= 200 && status < 500);
      }
    );

    req.on('error', (error) => {
      clearTimeout(timeout);
      console.log('[COMPUTE] checkKasmReady error:', error.message);
      resolve(false);
    });

    req.on('timeout', () => {
      clearTimeout(timeout);
      req.destroy();
      resolve(false);
    });

    req.end();
  });
}

// Fetch detailed progress from the VM's status.json endpoint
export async function fetchVMProgress(ip: string): Promise<VMProgress | null> {
  return new Promise((resolve) => {
    const timeout = setTimeout(() => {
      resolve(null);
    }, 3000);

    // Use HTTP on port 8080 (simple Python status server)
    const req = http.request(
      {
        hostname: ip,
        port: 8080,
        path: '/status.json',
        method: 'GET',
        timeout: 3000,
      },
      (res) => {
        clearTimeout(timeout);
        
        if (!res.statusCode || res.statusCode !== 200) {
          console.log('[COMPUTE] fetchVMProgress: status.json not available yet');
          resolve(null);
          return;
        }

        let data = '';
        res.on('data', (chunk) => {
          data += chunk;
        });

        res.on('end', () => {
          try {
            const json = JSON.parse(data);
            console.log('[COMPUTE] fetchVMProgress:', json);
            resolve(json);
          } catch (e) {
            console.log('[COMPUTE] fetchVMProgress parse error:', e);
            resolve(null);
          }
        });
      }
    );

    req.on('error', (error) => {
      clearTimeout(timeout);
      console.log('[COMPUTE] fetchVMProgress error:', error.message);
      resolve(null);
    });

    req.on('timeout', () => {
      clearTimeout(timeout);
      req.destroy();
      resolve(null);
    });

    req.end();
  });
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
