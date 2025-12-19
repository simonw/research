import { NextRequest, NextResponse } from 'next/server';
import { createVM, generateSessionId } from '@/lib/compute';

export async function POST(request: NextRequest) {
  try {
    const sessionId = generateSessionId();
    console.log('[API] Starting new session:', sessionId);

    const vmName = await createVM(sessionId);
    console.log('[API] VM created:', vmName);

    return NextResponse.json({
      sessionId,
      vmName,
      status: 'starting',
    });
  } catch (error) {
    console.error('[API] Error starting session:', error);
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to start session' },
      { status: 500 }
    );
  }
}
