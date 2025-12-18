import React, { useState } from 'react';
import { Card, Button, Badge } from '../atoms';

interface Backup {
  id: string;
  name: string;
  vmName: string;
  status: 'Completed' | 'In Progress' | 'Failed';
  size: string;
  createdAt: string;
  duration: string;
  location: string;
}

export const BackupPage: React.FC = () => {
  const [backups, setBackups] = useState<Backup[]>([
    {
      id: 'b1',
      name: 'Daily Backup - Dec 11',
      vmName: 'Ubuntu-20.04',
      status: 'Completed',
      size: '2.3 GB',
      createdAt: '2025-12-11 02:00',
      duration: '45 min',
      location: '/backups/ubuntu-20.04/daily',
    },
    {
      id: 'b2',
      name: 'Weekly Backup',
      vmName: 'CentOS-8',
      status: 'Completed',
      size: '3.8 GB',
      createdAt: '2025-12-08 03:00',
      duration: '62 min',
      location: '/backups/centos-8/weekly',
    },
    {
      id: 'b3',
      name: 'Backup - Pre Update',
      vmName: 'Windows-Server-2022',
      status: 'Completed',
      size: '7.2 GB',
      createdAt: '2025-12-05 22:30',
      duration: '95 min',
      location: '/backups/windows-server-2022/pre-update',
    },
    {
      id: 'b4',
      name: 'Current Backup',
      vmName: 'Ubuntu-20.04',
      status: 'In Progress',
      size: '1.5 GB (ongoing)',
      createdAt: '2025-12-11 15:30',
      duration: '12 min remaining',
      location: '/backups/ubuntu-20.04/current',
    },
  ]);

  const [selectedBackupId, setSelectedBackupId] = useState<string | null>(null);
  const [scheduleEnabled, setScheduleEnabled] = useState(true);
  const [backupSchedule, setBackupSchedule] = useState('daily');
  const [backupTime, setBackupTime] = useState('02:00');
  const [compression, setCompression] = useState('medium');
  const [backupPath, setBackupPath] = useState('/backups');
  const [retention, setRetention] = useState('30');

  const selectedBackup = backups.find(b => b.id === selectedBackupId);

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'Completed':
        return 'bg-green-100 text-green-800 border-green-300';
      case 'In Progress':
        return 'bg-blue-100 text-blue-800 border-blue-300';
      case 'Failed':
        return 'bg-red-100 text-red-800 border-red-300';
      default:
        return 'bg-gray-100 text-gray-800 border-gray-300';
    }
  };

  const handleStartBackup = () => {
    const newBackup: Backup = {
      id: `b${backups.length + 1}`,
      name: 'Manual Backup',
      vmName: 'Ubuntu-20.04',
      status: 'In Progress',
      size: '0 GB',
      createdAt: new Date().toLocaleString(),
      duration: 'Running...',
      location: backupPath,
    };
    setBackups([...backups, newBackup]);
  };

  const handleRestoreBackup = (backupId: string) => {
    if (window.confirm('Restore from this backup? Current VM data will be overwritten.')) {
      alert('Restore operation started. This may take several minutes.');
    }
  };

  const handleDeleteBackup = (backupId: string) => {
    if (window.confirm('Delete this backup permanently?')) {
      setBackups(backups.filter(b => b.id !== backupId));
      setSelectedBackupId(null);
    }
  };

  const totalSize = backups
    .filter(b => b.status === 'Completed')
    .reduce((sum, b) => sum + parseFloat(b.size), 0)
    .toFixed(1);

  return (
    <div className="space-y-4 p-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-800">Backup & Recovery</h1>
          <p className="text-gray-500">Manage backups and restore points for your VMs</p>
        </div>
        <Button
          onClick={handleStartBackup}
          className="bg-green-500 hover:bg-green-600"
        >
          💾 Start Backup Now
        </Button>
      </div>

      {/* Main Content */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Left: Backups List */}
        <div className="lg:col-span-2 space-y-4">
          {/* Backups List */}
          <Card className="p-4">
            <h2 className="font-semibold text-gray-800 mb-3">Recent Backups</h2>
            <div className="space-y-2 max-h-96 overflow-y-auto">
              {backups.map(backup => (
                <div
                  key={backup.id}
                  onClick={() => setSelectedBackupId(backup.id)}
                  className={`p-3 rounded-lg cursor-pointer transition border-2 ${
                    selectedBackupId === backup.id
                      ? 'bg-blue-100 border-blue-500'
                      : 'bg-gray-50 hover:bg-gray-100 border-gray-200'
                  }`}
                >
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="flex items-center gap-2">
                        <h3 className="font-semibold text-gray-800">{backup.name}</h3>
                        <Badge className={`text-xs ${getStatusColor(backup.status)}`}>
                          {backup.status}
                        </Badge>
                      </div>
                      <p className="text-sm text-gray-600">{backup.vmName}</p>
                      <div className="flex gap-4 mt-2 text-xs text-gray-500">
                        <span>📦 {backup.size}</span>
                        <span>🕐 {backup.createdAt}</span>
                        <span>⏱️ {backup.duration}</span>
                      </div>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </Card>
        </div>

        {/* Right: Configuration */}
        <div className="space-y-4">
          {/* Backup Details */}
          <Card className="p-4">
            <h2 className="font-semibold text-gray-800 mb-3">Backup Details</h2>
            {selectedBackup ? (
              <div className="space-y-3 text-sm">
                <div>
                  <p className="text-gray-500">Name</p>
                  <p className="font-semibold text-gray-800">{selectedBackup.name}</p>
                </div>
                <div>
                  <p className="text-gray-500">VM</p>
                  <p className="font-semibold text-gray-800">{selectedBackup.vmName}</p>
                </div>
                <div>
                  <p className="text-gray-500">Location</p>
                  <p className="font-semibold text-gray-800 break-words">{selectedBackup.location}</p>
                </div>
                <div>
                  <p className="text-gray-500">Size</p>
                  <p className="font-semibold text-gray-800">{selectedBackup.size}</p>
                </div>
                <div>
                  <p className="text-gray-500">Created</p>
                  <p className="font-semibold text-gray-800">{selectedBackup.createdAt}</p>
                </div>

                <div className="pt-3 border-t space-y-2">
                  {selectedBackup.status === 'Completed' && (
                    <>
                      <Button
                        onClick={() => handleRestoreBackup(selectedBackup.id)}
                        className="w-full bg-blue-500 hover:bg-blue-600"
                      >
                        ↶ Restore
                      </Button>
                      <Button
                        onClick={() => handleDeleteBackup(selectedBackup.id)}
                        className="w-full bg-red-500 hover:bg-red-600"
                      >
                        🗑️ Delete
                      </Button>
                    </>
                  )}
                </div>
              </div>
            ) : (
              <p className="text-gray-500 text-center py-6">Select a backup to view details</p>
            )}
          </Card>

          {/* Statistics */}
          <Card className="p-4">
            <h2 className="font-semibold text-gray-800 mb-3">Statistics</h2>
            <div className="space-y-2 text-sm">
              <div className="flex justify-between">
                <span className="text-gray-600">Total Backups:</span>
                <span className="font-semibold">{backups.filter(b => b.status === 'Completed').length}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-600">Total Size:</span>
                <span className="font-semibold">{totalSize} GB</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-600">In Progress:</span>
                <span className="font-semibold">{backups.filter(b => b.status === 'In Progress').length}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-600">Failed:</span>
                <span className="font-semibold text-red-600">{backups.filter(b => b.status === 'Failed').length}</span>
              </div>
            </div>
          </Card>
        </div>
      </div>

      {/* Backup Settings */}
      <Card className="p-4">
        <h2 className="font-semibold text-gray-800 mb-4">Backup Schedule</h2>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div>
            <label className="flex items-center gap-2 mb-2">
              <input
                type="checkbox"
                checked={scheduleEnabled}
                onChange={(e) => setScheduleEnabled(e.target.checked)}
                className="rounded"
              />
              <span className="text-sm font-semibold text-gray-700">Enable Automatic Backups</span>
            </label>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Schedule
            </label>
            <select
              value={backupSchedule}
              onChange={(e) => setBackupSchedule(e.target.value)}
              disabled={!scheduleEnabled}
              className="w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100"
            >
              <option value="hourly">Hourly</option>
              <option value="daily">Daily</option>
              <option value="weekly">Weekly</option>
              <option value="monthly">Monthly</option>
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Backup Time
            </label>
            <input
              type="time"
              value={backupTime}
              onChange={(e) => setBackupTime(e.target.value)}
              disabled={!scheduleEnabled}
              className="w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Compression Level
            </label>
            <select
              value={compression}
              onChange={(e) => setCompression(e.target.value)}
              className="w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
            >
              <option value="none">None</option>
              <option value="low">Low</option>
              <option value="medium">Medium</option>
              <option value="high">High</option>
              <option value="maximum">Maximum</option>
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Backup Location
            </label>
            <input
              type="text"
              value={backupPath}
              onChange={(e) => setBackupPath(e.target.value)}
              className="w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Retention (days)
            </label>
            <input
              type="number"
              value={retention}
              onChange={(e) => setRetention(e.target.value)}
              min="7"
              max="365"
              className="w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>
        </div>

        <div className="flex gap-2 mt-4">
          <Button className="bg-blue-500 hover:bg-blue-600">
            💾 Save Settings
          </Button>
          <Button className="bg-gray-400 hover:bg-gray-500">
            Reset to Defaults
          </Button>
        </div>
      </Card>
    </div>
  );
};
