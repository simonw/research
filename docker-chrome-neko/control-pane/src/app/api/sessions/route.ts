import { NextRequest, NextResponse } from 'next/server';
import { listNekoVMs } from '@/lib/compute';

export async function GET(request: NextRequest) {
  try {
    const vms = await listNekoVMs();
    
    // Transform VMs to session format
    const sessions = vms.map((vm) => ({
      vmName: vm.name,
      status: vm.status.toLowerCase(),
      ip: vm.ip,
      zone: vm.zone,
      createdAt: vm.createdAt ? new Date(vm.createdAt).getTime() : Date.now(),
    }));
    
    return NextResponse.json({ sessions });
  } catch (error) {
    console.error('[API] Error listing sessions:', error);
    return NextResponse.json(
      { error: 'Failed to list sessions' },
      { status: 500 }
    );
  }
}
