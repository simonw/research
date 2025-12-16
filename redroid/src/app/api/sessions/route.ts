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
    
    // Filter to only running/provisioning VMs and map to session-like objects
    const activeSessions = vms
      .filter(vm => vm.status === 'RUNNING' || vm.status === 'STAGING' || vm.status === 'PROVISIONING')
      .map(vm => ({
        vmName: vm.name,
        status: vm.status === 'RUNNING' ? 'running' : 'starting',
        ip: vm.ip,
        zone: vm.zone,
        createdAt: vm.createdAt ? new Date(vm.createdAt).getTime() : Date.now(),
      }));
    
    return NextResponse.json({ sessions: activeSessions });
  } catch (error) {
    console.error('[SESSIONS] Error listing sessions:', error);
    return NextResponse.json(
      { error: 'Failed to list sessions', details: String(error) },
      { status: 500 }
    );
  }
}
