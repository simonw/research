import { NextResponse } from 'next/server';
import { getVMStatus, checkKasmReady } from '@/lib/compute';

// GET /api/session/[id] - Get session status by querying GCP directly
// The ID can be a full session ID or the vmName
export async function GET(
  request: Request,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const { id } = await params;
    
    // Support both full session ID and vmName
    const vmName = id.startsWith('redroid-') ? id : `redroid-${id.slice(0, 8)}`;
    
    // Query GCP directly for VM status
    const vmStatus = await getVMStatus(vmName);
    
    if (vmStatus.status === 'DELETED' || vmStatus.status === 'NOT_FOUND') {
      return NextResponse.json(
        { error: 'Session not found or already deleted' },
        { status: 404 }
      );
    }
    
    // Determine session status based on VM status
    let sessionStatus: 'starting' | 'running' | 'ended' | 'error' = 'starting';
    let kasmReady = false;
    
    if (vmStatus.status === 'RUNNING' && vmStatus.ip) {
      // Check if Kasm/noVNC is ready
      kasmReady = await checkKasmReady(vmStatus.ip);
      sessionStatus = kasmReady ? 'running' : 'starting';
    } else if (vmStatus.status === 'TERMINATED' || vmStatus.status === 'STOPPED') {
      sessionStatus = 'ended';
    } else if (vmStatus.status === 'STAGING' || vmStatus.status === 'PROVISIONING') {
      sessionStatus = 'starting';
    }
    
    // Build a session-like response from GCP data
    const session = {
      id,
      userId: 'unknown', // We don't store this in GCP
      vmName,
      zone: process.env.GCP_ZONE || 'us-central1-a',
      ip: vmStatus.ip,
      status: sessionStatus,
      createdAt: Date.now(), // We could get this from VM metadata if needed
      expiresAt: Date.now() + 60 * 60 * 1000, // TTL from config
      kasmReady,
    };
    
    return NextResponse.json({
      session,
      url: session.ip && session.status === 'running' 
        ? `https://${session.ip}/vnc.html` 
        : null,
    });
  } catch (error) {
    console.error('[SESSION STATUS] Error getting session status:', error);
    return NextResponse.json(
      { error: 'Failed to get session status', details: String(error) },
      { status: 500 }
    );
  }
}
