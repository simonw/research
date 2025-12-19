import { NextRequest, NextResponse } from 'next/server';
import { getVMStatus, fetchVMProgress } from '@/lib/compute';

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ vmName: string }> }
) {
  const { vmName } = await params;

  try {
    const vmStatus = await getVMStatus(vmName);
    
    let progress = null;
    let url = null;
    
    if (vmStatus.ip) {
      progress = await fetchVMProgress(vmStatus.ip);
      
      // Use tunnel URL if available, otherwise direct IP
      if (progress?.tunnelUrl) {
        url = progress.tunnelUrl;
      } else {
        url = `http://${vmStatus.ip}:8081`;
      }
    }

    return NextResponse.json({
      session: {
        vmName,
        status: vmStatus.status.toLowerCase(),
        ip: vmStatus.ip,
      },
      progress,
      url,
      cdpAgentUrl: progress?.cdpAgentUrl || (vmStatus.ip ? `http://${vmStatus.ip}:3001` : null),
    });
  } catch (error) {
    console.error('[API] Error getting session status:', error);
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to get session status' },
      { status: 500 }
    );
  }
}
