import { NextResponse } from 'next/server';
import { getVMStatus, checkKasmReady, fetchVMProgress } from '@/lib/compute';

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
    let progress = null;
    
    if (vmStatus.status === 'RUNNING' && vmStatus.ip) {
      // Fetch detailed progress from the VM
      progress = await fetchVMProgress(vmStatus.ip);
      
      // Check if emulator is ready (status.json shows ready stage)
      if (progress?.stage === 'ready') {
        kasmReady = true;
        sessionStatus = 'running';
      } else {
        // Fall back to checking ws-scrcpy endpoint
        kasmReady = await checkKasmReady(vmStatus.ip);
        sessionStatus = kasmReady ? 'running' : 'starting';
      }
    } else if (vmStatus.status === 'TERMINATED' || vmStatus.status === 'STOPPED') {
      sessionStatus = 'ended';
    } else if (vmStatus.status === 'STAGING' || vmStatus.status === 'PROVISIONING') {
      sessionStatus = 'starting';
      // Provide early stage progress when VM is still provisioning
      progress = {
        stage: 'provisioning',
        step: 0,
        totalSteps: 10,
        message: 'Provisioning virtual machine...',
        percent: 0,
      };
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
    
    // Build stream URL with auto-connect parameters
    // ws-scrcpy uses URL hash params: #!action=stream&udid=...&player=...&ws=...
    const buildStreamUrl = (baseUrl: string): string => {
      const udid = 'emulator-5554';  // Default redroid device ID
      
      // Build the WebSocket proxy URL (required for streaming)
      const wsProxyParams = new URLSearchParams({
        action: 'proxy-adb',
        remote: 'tcp:8886',
        udid: udid,
      });
      const wsUrl = `wss://${new URL(baseUrl).host}/?${wsProxyParams.toString()}`;
      
      // Build the stream URL hash params
      const params = new URLSearchParams({
        action: 'stream',
        udid: udid,
        player: 'webcodecs',     // Best performance in modern browsers
        ws: wsUrl,
      });
      return `${baseUrl}/#!${params.toString()}`;
    };

    const baseUrl = progress?.tunnelUrl || (session.ip && session.status === 'running' 
      ? `http://${session.ip}:8000`  // Fallback to direct ws-scrcpy
      : null);

    return NextResponse.json({
      session,
      url: baseUrl ? buildStreamUrl(baseUrl) : null,
      progress,
    });
  } catch (error) {
    console.error('[SESSION STATUS] Error getting session status:', error);
    return NextResponse.json(
      { error: 'Failed to get session status', details: String(error) },
      { status: 500 }
    );
  }
}
