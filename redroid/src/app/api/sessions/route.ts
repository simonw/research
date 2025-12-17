import { NextResponse } from 'next/server';
import { listRedroidVMs, deleteVM } from '@/lib/compute';

// GET /api/sessions - List all active redroid sessions from GCP (source of truth)
export async function GET() {
  try {
    const vms = await listRedroidVMs();
    
    // Garbage collect: delete any stopped/terminated VMs in the background
    // These shouldn't exist with the new auto-delete, but clean up legacy VMs
    const stoppedVMs = vms.filter(vm => 
      vm.status === 'TERMINATED' || vm.status === 'STOPPED' || vm.status === 'SUSPENDED'
    );
    if (stoppedVMs.length > 0) {
      console.log('[SESSIONS] Garbage collecting stopped VMs:', stoppedVMs.map(vm => vm.name));
      // Delete in background, don't await
      Promise.all(stoppedVMs.map(vm => deleteVM(vm.name).catch(e => 
        console.error(`[SESSIONS] Failed to delete ${vm.name}:`, e)
      )));
    }
    
    // Map GCP VM status to user-friendly status
    const mapStatus = (gcpStatus: string): string => {
      switch (gcpStatus) {
        case 'RUNNING': return 'running';
        case 'STAGING': return 'staging';
        case 'PROVISIONING': return 'provisioning';
        case 'STOPPING': return 'stopping';
        case 'TERMINATED': return 'terminated';
        case 'STOPPED': return 'stopped';
        case 'SUSPENDED': return 'suspended';
        case 'SUSPENDING': return 'suspending';
        default: return gcpStatus.toLowerCase();
      }
    };

    // Return ALL VMs so UI can show starting, stopping, etc. statuses
    const allSessions = vms.map(vm => ({
      vmName: vm.name,
      status: mapStatus(vm.status),
      gcpStatus: vm.status, // Include raw GCP status for debugging
      ip: vm.ip,
      zone: vm.zone,
      createdAt: vm.createdAt ? new Date(vm.createdAt).getTime() : Date.now(),
    }));
    
    return NextResponse.json({ sessions: allSessions });
  } catch (error) {
    console.error('[SESSIONS] Error listing sessions:', error);
    return NextResponse.json(
      { error: 'Failed to list sessions', details: String(error) },
      { status: 500 }
    );
  }
}
