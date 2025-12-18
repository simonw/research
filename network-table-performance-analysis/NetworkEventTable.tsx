// NetworkEventTable.tsx - Optimized implementation
import React, { memo, useMemo, useCallback } from 'react';
import { FixedSizeList as List } from 'react-window';

interface NetworkEvent {
  id: string;
  url: string;
  method: string;
  status: number;
  size: number;
  timestamp: number;
  type: string;
}

interface NetworkEventRowProps {
  index: number;
  style: React.CSSProperties;
  data: NetworkEvent[];
}

// Memoized row component - prevents unnecessary re-renders
const NetworkEventRow = memo<NetworkEventRowProps>(({ index, style, data }) => {
  const event = data[index];
  
  // Memoize expensive formatting
  const formattedTime = useMemo(() => {
    return new Date(event.timestamp).toLocaleTimeString();
  }, [event.timestamp]);
  
  const formattedSize = useMemo(() => {
    return event.size > 1024 
      ? `${(event.size / 1024).toFixed(1)}KB`
      : `${event.size}B`;
  }, [event.size]);

  return (
    <div 
      style={style} 
      className="flex border-b border-gray-200 hover:bg-gray-50"
    >
      <div className="w-20 p-2 text-xs text-gray-500">{formattedTime}</div>
      <div className="w-12 p-2 text-xs font-mono">{event.method}</div>
      <div className="flex-1 p-2 text-sm truncate">{event.url}</div>
      <div className="w-16 p-2 text-xs">{event.status}</div>
      <div className="w-20 p-2 text-xs text-gray-500">{formattedSize}</div>
      <div className="w-16 p-2 text-xs text-gray-500">{event.type}</div>
    </div>
  );
});

NetworkEventRow.displayName = 'NetworkEventRow';

interface NetworkEventTableProps {
  events: NetworkEvent[];
  height?: number;
}

// Main table component with virtualization
export function NetworkEventTable({ 
  events, 
  height = 400 
}: NetworkEventTableProps) {
  
  // Memoize processed data to prevent unnecessary re-computation
  const processedEvents = useMemo(() => {
    return events.map(event => ({
      ...event,
      // Add any computed properties here
    }));
  }, [events]);
  
  // Memoize item size for consistency
  const itemSize = 40; // Fixed row height
  
  // Handle empty state
  if (processedEvents.length === 0) {
    return (
      <div 
        className="flex items-center justify-center border rounded"
        style={{ height }}
      >
        <p className="text-gray-500">No network events captured yet</p>
      </div>
    );
  }

  return (
    <div className="border rounded">
      {/* Header */}
      <div className="flex border-b bg-gray-50 font-medium text-xs">
        <div className="w-20 p-2">Time</div>
        <div className="w-12 p-2">Method</div>
        <div className="flex-1 p-2">URL</div>
        <div className="w-16 p-2">Status</div>
        <div className="w-20 p-2">Size</div>
        <div className="w-16 p-2">Type</div>
      </div>
      
      {/* Virtualized list */}
      <List
        height={height - 40} // Subtract header height
        itemCount={processedEvents.length}
        itemSize={itemSize}
        itemData={processedEvents}
        overscanCount={5} // Render extra items for smooth scrolling
      >
        {NetworkEventRow}
      </List>
      
      {/* Footer with count */}
      <div className="border-t bg-gray-50 px-2 py-1 text-xs text-gray-500">
        {processedEvents.length} events
      </div>
    </div>
  );
}

// Hook for managing network events with performance optimizations
export function useNetworkEvents() {
  const [events, setEvents] = React.useState<NetworkEvent[]>([]);
  
  // Memoize event handlers
  const addEvent = useCallback((event: NetworkEvent) => {
    setEvents(prev => [event, ...prev].slice(0, 10000)); // Keep last 10k events
  }, []);
  
  const clearEvents = useCallback(() => {
    setEvents([]);
  }, []);
  
  // Memoize filtered/sorted views
  const filteredEvents = useMemo(() => {
    return events; // Add filtering logic here if needed
  }, [events]);
  
  return {
    events: filteredEvents,
    addEvent,
    clearEvents,
    count: events.length
  };
}
