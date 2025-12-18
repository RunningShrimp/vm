import React, { useState } from 'react';
import { Card, Button, Badge } from '../atoms';

interface Log {
  id: string;
  timestamp: string;
  level: 'INFO' | 'WARNING' | 'ERROR' | 'DEBUG';
  source: string;
  message: string;
}

export const LogViewerPage: React.FC = () => {
  const [logs, setLogs] = useState<Log[]>([
    {
      id: '1',
      timestamp: '2025-12-11 10:45:32',
      level: 'INFO',
      source: 'VMManager',
      message: 'VM "Ubuntu-20.04" started successfully',
    },
    {
      id: '2',
      timestamp: '2025-12-11 10:44:15',
      level: 'WARNING',
      source: 'NetworkService',
      message: 'High latency detected on interface eth0 (120ms)',
    },
    {
      id: '3',
      timestamp: '2025-12-11 10:43:22',
      level: 'DEBUG',
      source: 'StorageManager',
      message: 'Cache invalidation triggered for volume /dev/vda1',
    },
    {
      id: '4',
      timestamp: '2025-12-11 10:42:05',
      level: 'ERROR',
      source: 'BackupService',
      message: 'Backup failed: Insufficient disk space (required: 50GB, available: 25GB)',
    },
    {
      id: '5',
      timestamp: '2025-12-11 10:40:30',
      level: 'INFO',
      source: 'MonitoringService',
      message: 'CPU usage exceeds 80% threshold',
    },
    {
      id: '6',
      timestamp: '2025-12-11 10:38:12',
      level: 'WARNING',
      source: 'MemoryManager',
      message: 'Memory swap usage at 45%',
    },
    {
      id: '7',
      timestamp: '2025-12-11 10:35:48',
      level: 'INFO',
      source: 'VMManager',
      message: 'VM "CentOS-8" migration started',
    },
  ]);

  const [filterLevel, setFilterLevel] = useState<string>('ALL');
  const [filterSource, setFilterSource] = useState<string>('ALL');
  const [searchText, setSearchText] = useState('');
  const [selectedLogId, setSelectedLogId] = useState<string | null>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  const selectedLog = logs.find(l => l.id === selectedLogId);

  const getLevelColor = (level: string) => {
    switch (level) {
      case 'INFO':
        return 'bg-blue-100 text-blue-800';
      case 'WARNING':
        return 'bg-yellow-100 text-yellow-800';
      case 'ERROR':
        return 'bg-red-100 text-red-800';
      case 'DEBUG':
        return 'bg-gray-100 text-gray-800';
      default:
        return 'bg-gray-100 text-gray-800';
    }
  };

  const getLevelIcon = (level: string) => {
    switch (level) {
      case 'INFO':
        return 'ℹ️';
      case 'WARNING':
        return '⚠️';
      case 'ERROR':
        return '❌';
      case 'DEBUG':
        return '🐛';
      default:
        return '📝';
    }
  };

  const filteredLogs = logs.filter(log => {
    const levelMatch = filterLevel === 'ALL' || log.level === filterLevel;
    const sourceMatch = filterSource === 'ALL' || log.source === filterSource;
    const searchMatch = searchText === '' || 
      log.message.toLowerCase().includes(searchText.toLowerCase()) ||
      log.source.toLowerCase().includes(searchText.toLowerCase());
    return levelMatch && sourceMatch && searchMatch;
  });

  const sources = ['ALL', ...new Set(logs.map(l => l.source))];
  const levels = ['ALL', 'INFO', 'WARNING', 'ERROR', 'DEBUG'];

  const handleClearLogs = () => {
    if (window.confirm('Clear all logs? This cannot be undone.')) {
      setLogs([]);
      setSelectedLogId(null);
    }
  };

  const handleExportLogs = () => {
    const logText = filteredLogs
      .map(l => `[${l.timestamp}] [${l.level}] [${l.source}] ${l.message}`)
      .join('\n');
    
    const element = document.createElement('a');
    element.setAttribute('href', 'data:text/plain;charset=utf-8,' + encodeURIComponent(logText));
    element.setAttribute('download', `vm-logs-${new Date().toISOString()}.txt`);
    element.style.display = 'none';
    document.body.appendChild(element);
    element.click();
    document.body.removeChild(element);
  };

  return (
    <div className="space-y-4 p-4 h-full flex flex-col">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold text-gray-800">Log Viewer</h1>
        <p className="text-gray-500">View and analyze system logs</p>
      </div>

      {/* Filters */}
      <Card className="p-4">
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Level
            </label>
            <select
              value={filterLevel}
              onChange={(e) => setFilterLevel(e.target.value)}
              className="w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 text-sm"
            >
              {levels.map(level => (
                <option key={level} value={level}>{level}</option>
              ))}
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Source
            </label>
            <select
              value={filterSource}
              onChange={(e) => setFilterSource(e.target.value)}
              className="w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 text-sm"
            >
              {sources.map(source => (
                <option key={source} value={source}>{source}</option>
              ))}
            </select>
          </div>

          <div className="md:col-span-2">
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Search
            </label>
            <input
              type="text"
              value={searchText}
              onChange={(e) => setSearchText(e.target.value)}
              placeholder="Search logs..."
              className="w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 text-sm"
            />
          </div>
        </div>

        <div className="flex gap-2 mt-3 flex-wrap">
          <label className="flex items-center gap-2 text-sm cursor-pointer">
            <input
              type="checkbox"
              checked={autoScroll}
              onChange={(e) => setAutoScroll(e.target.checked)}
              className="rounded"
            />
            Auto-scroll
          </label>
          <Button
            onClick={handleExportLogs}
            className="text-sm bg-green-500 hover:bg-green-600"
          >
            📥 Export
          </Button>
          <Button
            onClick={handleClearLogs}
            className="text-sm bg-red-500 hover:bg-red-600"
          >
            🗑️ Clear
          </Button>
        </div>
      </Card>

      {/* Logs View */}
      <div className="flex-1 grid grid-cols-1 lg:grid-cols-3 gap-4 min-h-0">
        {/* Logs List */}
        <div className="lg:col-span-2 flex flex-col min-h-0">
          <Card className="p-4 overflow-hidden flex flex-col">
            <h2 className="font-semibold text-gray-800 mb-2">Logs ({filteredLogs.length})</h2>
            <div className="overflow-y-auto flex-1 space-y-1">
              {filteredLogs.length === 0 ? (
                <div className="text-center py-8 text-gray-500">
                  <p>No logs match the current filters</p>
                </div>
              ) : (
                filteredLogs.map(log => (
                  <div
                    key={log.id}
                    onClick={() => setSelectedLogId(log.id)}
                    className={`p-2 rounded-lg cursor-pointer transition text-sm border-l-4 ${
                      selectedLogId === log.id
                        ? 'bg-blue-100 border-blue-500'
                        : 'bg-gray-50 hover:bg-gray-100 border-gray-300'
                    }`}
                    style={{
                      borderLeftColor: 
                        log.level === 'ERROR' ? '#ef4444' :
                        log.level === 'WARNING' ? '#f59e0b' :
                        log.level === 'INFO' ? '#3b82f6' :
                        '#6b7280'
                    }}
                  >
                    <div className="flex items-start gap-2">
                      <span className="text-lg flex-shrink-0">{getLevelIcon(log.level)}</span>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 flex-wrap">
                          <span className="text-xs text-gray-500">{log.timestamp}</span>
                          <Badge className={`text-xs ${getLevelColor(log.level)}`}>
                            {log.level}
                          </Badge>
                          <span className="text-xs font-semibold text-gray-700">{log.source}</span>
                        </div>
                        <p className="text-gray-800 line-clamp-2">{log.message}</p>
                      </div>
                    </div>
                  </div>
                ))
              )}
            </div>
          </Card>
        </div>

        {/* Log Details */}
        <div>
          <Card className="p-4 h-full overflow-auto">
            <h2 className="font-semibold text-gray-800 mb-3">Log Details</h2>
            {selectedLog ? (
              <div className="space-y-4 text-sm">
                <div>
                  <p className="text-xs text-gray-500 uppercase font-semibold mb-1">Timestamp</p>
                  <p className="font-mono text-gray-800">{selectedLog.timestamp}</p>
                </div>
                <div>
                  <p className="text-xs text-gray-500 uppercase font-semibold mb-1">Level</p>
                  <Badge className={`text-xs ${getLevelColor(selectedLog.level)}`}>
                    {getLevelIcon(selectedLog.level)} {selectedLog.level}
                  </Badge>
                </div>
                <div>
                  <p className="text-xs text-gray-500 uppercase font-semibold mb-1">Source</p>
                  <p className="font-mono text-gray-800">{selectedLog.source}</p>
                </div>
                <div>
                  <p className="text-xs text-gray-500 uppercase font-semibold mb-1">Message</p>
                  <p className="text-gray-800 break-words whitespace-pre-wrap bg-gray-50 p-2 rounded">
                    {selectedLog.message}
                  </p>
                </div>

                <div className="pt-4 border-t space-y-2">
                  <Button className="w-full bg-blue-500 hover:bg-blue-600 text-sm">
                    📋 Copy
                  </Button>
                  <Button className="w-full bg-gray-400 hover:bg-gray-500 text-sm">
                    🔍 Similar Logs
                  </Button>
                </div>
              </div>
            ) : (
              <p className="text-gray-500 text-center py-6">Select a log entry to view details</p>
            )}
          </Card>
        </div>
      </div>
    </div>
  );
};
