import { NextResponse } from 'next/server';
import { deleteVM } from '@/lib/compute';
import { EndSessionResponse } from '@/lib/types';

// End a session by deleting the VM directly using vmName
// This doesn't rely on server-side session state - GCP is the source of truth
export async function POST(request: Request) {
  try {
    const body = await request.json();
    const { vmName, sessionId } = body;
    
    // Support both vmName (new) and sessionId (legacy) for backwards compatibility
    let targetVmName = vmName;
    
    if (!targetVmName && sessionId) {
      // Derive vmName from sessionId for backwards compatibility
      targetVmName = `redroid-${sessionId.slice(0, 8)}`;
    }
    
    if (!targetVmName) {
      return NextResponse.json(
        { error: 'vmName or sessionId is required' },
        { status: 400 }
      );
    }
    
    // Validate that this is a redroid VM
    if (!targetVmName.startsWith('redroid-')) {
      return NextResponse.json(
        { error: 'Invalid VM name - must be a redroid VM' },
        { status: 400 }
      );
    }
    
    console.log('[END SESSION] Deleting VM:', targetVmName);
    
    // Delete the VM directly - no session lookup needed
    await deleteVM(targetVmName);
    
    const response: EndSessionResponse = {
      success: true,
      message: `VM ${targetVmName} deleted successfully`,
    };
    
    return NextResponse.json(response);
  } catch (error) {
    console.error('[END SESSION] Error ending session:', error);
    return NextResponse.json(
      { error: 'Failed to end session', details: String(error) },
      { status: 500 }
    );
  }
}
