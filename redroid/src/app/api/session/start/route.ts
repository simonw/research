import { NextResponse } from 'next/server';
import { createVM, generateSessionId, getVMConfig } from '@/lib/compute';
import { StartSessionResponse } from '@/lib/types';

export async function POST(request: Request) {
  console.log('[START SESSION] Request received');
  
  try {
    const body = await request.json().catch(() => ({}));
    const userId = body.userId || 'anonymous';
    console.log('[START SESSION] User ID:', userId);
    
    const sessionId = generateSessionId();
    const config = getVMConfig();
    console.log('[START SESSION] Session ID:', sessionId);
    console.log('[START SESSION] VM Config:', JSON.stringify(config, null, 2));
    
    // Create VM
    console.log('[START SESSION] Creating VM...');
    const vmName = await createVM(sessionId);
    console.log('[START SESSION] VM created:', vmName);
    
    // Note: We no longer store sessions in memory.
    // GCP is the source of truth - query /api/sessions to list active VMs
    
    const response: StartSessionResponse = {
      sessionId,
      status: 'starting',
    };
    
    return NextResponse.json(response);
  } catch (error) {
    console.error('[START SESSION] Error:', error);
    return NextResponse.json(
      { error: 'Failed to start session', details: String(error) },
      { status: 500 }
    );
  }
}
