import { NextResponse } from 'next/server';
import { listRedroidVMs } from '@/lib/compute';

// GET /api/sessions - List all active redroid sessions from GCP (source of truth)
export async function GET() {
  try {
    const vms = await listRedroidVMs();
    
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
