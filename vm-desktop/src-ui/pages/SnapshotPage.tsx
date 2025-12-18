import React, { useState } from 'react';
import { Card, Button, Badge } from '../atoms';

interface Snapshot {
  id: string;
  name: string;
  vmName: string;
  createdAt: string;
  size: string;
  description: string;
  isActive: boolean;
}

export const SnapshotPage: React.FC = () => {
  const [snapshots, setSnapshots] = useState<Snapshot[]>([
    {
      id: 's1',
      name: 'Before Update',
      vmName: 'Ubuntu-20.04',
      createdAt: '2025-12-10 14:32',
      size: '2.5 GB',
      description: 'Clean state before system update',
      isActive: false,
    },
    {
      id: 's2',
      name: 'Current State',
      vmName: 'Ubuntu-20.04',
      createdAt: '2025-12-11 09:15',
      size: '3.2 GB',
      description: 'After applying security patches',
      isActive: true,
    },
    {
      id: 's3',
      name: 'Development Setup',
      vmName: 'CentOS-8',
      createdAt: '2025-12-08 16:45',
      size: '5.8 GB',
      description: 'Development environment with all tools',
      isActive: false,
    },
    {
      id: 's4',
      name: 'Initial Install',
      vmName: 'Windows-Server-2022',
      createdAt: '2025-12-05 10:20',
      size: '8.1 GB',
      description: 'Fresh Windows Server installation',
      isActive: true,
    },
  ]);

  const [selectedSnapshotId, setSelectedSnapshotId] = useState<string | null>(null);
  const [newSnapshotName, setNewSnapshotName] = useState('');
  const [newSnapshotDesc, setNewSnapshotDesc] = useState('');
  const [showCreateForm, setShowCreateForm] = useState(false);

  const selectedSnapshot = snapshots.find(s => s.id === selectedSnapshotId);

  const handleCreateSnapshot = () => {
    if (!newSnapshotName.trim()) return;

    const newSnapshot: Snapshot = {
      id: `s${snapshots.length + 1}`,
      name: newSnapshotName,
      vmName: 'Ubuntu-20.04',
      createdAt: new Date().toLocaleString(),
      size: '2.0 GB',
      description: newSnapshotDesc,
      isActive: false,
    };

    setSnapshots([...snapshots, newSnapshot]);
    setNewSnapshotName('');
    setNewSnapshotDesc('');
    setShowCreateForm(false);
  };

  const handleRestoreSnapshot = (snapshotId: string) => {
    if (window.confirm('Restore this snapshot? Current VM state will be lost.')) {
      setSnapshots(snapshots.map(s => ({
        ...s,
        isActive: s.id === snapshotId,
      })));
    }
  };

  const handleDeleteSnapshot = (snapshotId: string) => {
    if (window.confirm('Delete this snapshot permanently?')) {
      setSnapshots(snapshots.filter(s => s.id !== snapshotId));
      setSelectedSnapshotId(null);
    }
  };

  const handleCloneSnapshot = (snapshotId: string) => {
    const source = snapshots.find(s => s.id === snapshotId);
    if (source) {
      const cloned: Snapshot = {
        ...source,
        id: `s${snapshots.length + 1}`,
        name: `${source.name} (Clone)`,
        isActive: false,
      };
      setSnapshots([...snapshots, cloned]);
    }
  };

  return (
    <div className="space-y-4 p-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-800">Snapshots</h1>
          <p className="text-gray-500">Manage VM snapshots and restore points</p>
        </div>
        <Button
          onClick={() => setShowCreateForm(!showCreateForm)}
          className="bg-blue-500 hover:bg-blue-600"
        >
          {showCreateForm ? 'Cancel' : '📸 Create Snapshot'}
        </Button>
      </div>

      {/* Create Form */}
      {showCreateForm && (
        <Card className="p-4 bg-blue-50 border-2 border-blue-300">
          <h3 className="font-semibold text-gray-800 mb-3">Create New Snapshot</h3>
          <div className="space-y-3">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                Snapshot Name
              </label>
              <input
                type="text"
                value={newSnapshotName}
                onChange={(e) => setNewSnapshotName(e.target.value)}
                placeholder="e.g., Before Update"
                className="w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                Description (optional)
              </label>
              <textarea
                value={newSnapshotDesc}
                onChange={(e) => setNewSnapshotDesc(e.target.value)}
                placeholder="Add notes about this snapshot..."
                rows={3}
                className="w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>
            <div className="flex gap-2">
              <Button
                onClick={handleCreateSnapshot}
                className="bg-green-500 hover:bg-green-600"
              >
                Create
              </Button>
              <Button
                onClick={() => setShowCreateForm(false)}
                className="bg-gray-400 hover:bg-gray-500"
              >
                Cancel
              </Button>
            </div>
          </div>
        </Card>
      )}

      {/* Main Content */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Snapshots List */}
        <div className="lg:col-span-2">
          <Card className="p-4">
            <h2 className="font-semibold text-gray-800 mb-3">All Snapshots</h2>
            <div className="space-y-2 max-h-96 overflow-y-auto">
              {snapshots.length === 0 ? (
                <div className="text-center py-8 text-gray-500">
                  <p>No snapshots created yet</p>
                  <p className="text-sm">Create a snapshot to preserve your VM state</p>
                </div>
              ) : (
                snapshots.map(snapshot => (
                  <div
                    key={snapshot.id}
                    onClick={() => setSelectedSnapshotId(snapshot.id)}
                    className={`p-3 rounded-lg cursor-pointer transition ${
                      selectedSnapshotId === snapshot.id
                        ? 'bg-blue-100 border-2 border-blue-500'
                        : 'bg-gray-50 hover:bg-gray-100 border-2 border-gray-200'
                    }`}
                  >
                    <div className="flex items-start justify-between">
                      <div className="flex-1">
                        <div className="flex items-center gap-2">
                          <h3 className="font-semibold text-gray-800">{snapshot.name}</h3>
                          {snapshot.isActive && (
                            <Badge className="bg-green-500 text-white">Active</Badge>
                          )}
                        </div>
                        <p className="text-sm text-gray-600">{snapshot.vmName}</p>
                        <p className="text-xs text-gray-500 mt-1">{snapshot.createdAt}</p>
                      </div>
                      <div className="text-right">
                        <p className="text-sm font-semibold text-gray-700">{snapshot.size}</p>
                      </div>
                    </div>
                    {snapshot.description && (
                      <p className="text-xs text-gray-600 mt-2 italic">
                        "{snapshot.description}"
                      </p>
                    )}
                  </div>
                ))
              )}
            </div>
          </Card>
        </div>

        {/* Details Panel */}
        <div>
          <Card className="p-4 h-full">
            <h2 className="font-semibold text-gray-800 mb-3">Details</h2>
            {selectedSnapshot ? (
              <div className="space-y-4">
                <div>
                  <p className="text-xs text-gray-500 uppercase">Name</p>
                  <p className="font-semibold text-gray-800">{selectedSnapshot.name}</p>
                </div>
                <div>
                  <p className="text-xs text-gray-500 uppercase">VM</p>
                  <p className="font-semibold text-gray-800">{selectedSnapshot.vmName}</p>
                </div>
                <div>
                  <p className="text-xs text-gray-500 uppercase">Created</p>
                  <p className="font-semibold text-gray-800">{selectedSnapshot.createdAt}</p>
                </div>
                <div>
                  <p className="text-xs text-gray-500 uppercase">Size</p>
                  <p className="font-semibold text-gray-800">{selectedSnapshot.size}</p>
                </div>
                <div>
                  <p className="text-xs text-gray-500 uppercase">Status</p>
                  <p className="font-semibold text-gray-800">
                    {selectedSnapshot.isActive ? '✓ Active' : 'Inactive'}
                  </p>
                </div>
                {selectedSnapshot.description && (
                  <div>
                    <p className="text-xs text-gray-500 uppercase">Description</p>
                    <p className="text-sm text-gray-700">{selectedSnapshot.description}</p>
                  </div>
                )}

                {/* Actions */}
                <div className="space-y-2 pt-4 border-t">
                  {!selectedSnapshot.isActive && (
                    <Button
                      onClick={() => handleRestoreSnapshot(selectedSnapshot.id)}
                      className="w-full bg-green-500 hover:bg-green-600"
                    >
                      ↶ Restore
                    </Button>
                  )}
                  <Button
                    onClick={() => handleCloneSnapshot(selectedSnapshot.id)}
                    className="w-full bg-purple-500 hover:bg-purple-600"
                  >
                    📋 Clone
                  </Button>
                  <Button
                    onClick={() => handleDeleteSnapshot(selectedSnapshot.id)}
                    className="w-full bg-red-500 hover:bg-red-600"
                  >
                    🗑️ Delete
                  </Button>
                </div>
              </div>
            ) : (
              <div className="text-center py-8 text-gray-500">
                <p>Select a snapshot to view details</p>
              </div>
            )}
          </Card>
        </div>
      </div>

      {/* Statistics */}
      <div className="grid grid-cols-4 gap-4">
        <Card className="p-4 text-center">
          <p className="text-2xl font-bold text-gray-800">{snapshots.length}</p>
          <p className="text-xs text-gray-600">Total Snapshots</p>
        </Card>
        <Card className="p-4 text-center">
          <p className="text-2xl font-bold text-gray-800">
            {snapshots.reduce((sum, s) => sum + parseFloat(s.size), 0).toFixed(1)}
          </p>
          <p className="text-xs text-gray-600">Total Size (GB)</p>
        </Card>
        <Card className="p-4 text-center">
          <p className="text-2xl font-bold text-gray-800">
            {snapshots.filter(s => s.isActive).length}
          </p>
          <p className="text-xs text-gray-600">Active VMs</p>
        </Card>
        <Card className="p-4 text-center">
          <p className="text-2xl font-bold text-gray-800">
            {new Set(snapshots.map(s => s.vmName)).size}
          </p>
          <p className="text-xs text-gray-600">VMs</p>
        </Card>
      </div>
    </div>
  );
};
