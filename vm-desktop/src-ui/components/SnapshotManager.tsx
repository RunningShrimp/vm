import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { 
  PhotoIcon, 
  ArrowDownTrayIcon, 
  TrashIcon,
  ClockIcon,
  DocumentTextIcon,
  ExclamationTriangleIcon
} from '@heroicons/react/24/outline';

interface Snapshot {
  id: string;
  name: string;
  description: string;
  created_at: string;
  size_mb: number;
}

interface SnapshotManagerProps {
  vmId: string;
  vmName: string;
}

export const SnapshotManager: React.FC<SnapshotManagerProps> = ({ vmId, vmName }) => {
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isCreating, setIsCreating] = useState(false);
  const [isRestoring, setIsRestoring] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [newSnapshotName, setNewSnapshotName] = useState('');
  const [newSnapshotDescription, setNewSnapshotDescription] = useState('');

  useEffect(() => {
    loadSnapshots();
  }, [vmId]);

  const loadSnapshots = async () => {
    setIsLoading(true);
    setError(null);
    
    try {
      const snapshotList = await invoke<any[]>('list_snapshots', { id: vmId });
      const formattedSnapshots: Snapshot[] = snapshotList.map(snapshot => ({
        id: snapshot.id,
        name: snapshot.name,
        description: snapshot.description,
        created_at: snapshot.created_at,
        size_mb: snapshot.size_mb || 0,
      }));
      setSnapshots(formattedSnapshots);
    } catch (err) {
      setError(`Failed to load snapshots: ${err}`);
    } finally {
      setIsLoading(false);
    }
  };

  const createSnapshot = async () => {
    if (!newSnapshotName.trim()) {
      setError('Snapshot name is required');
      return;
    }

    setIsCreating(true);
    setError(null);

    try {
      await invoke('create_snapshot', {
        id: vmId,
        name: newSnapshotName,
        description: newSnapshotDescription,
      });
      
      // Reset form
      setNewSnapshotName('');
      setNewSnapshotDescription('');
      setShowCreateForm(false);
      
      // Reload snapshots
      await loadSnapshots();
    } catch (err) {
      setError(`Failed to create snapshot: ${err}`);
    } finally {
      setIsCreating(false);
    }
  };

  const restoreSnapshot = async (snapshotId: string) => {
    if (!confirm('Are you sure you want to restore this snapshot? The current VM state will be lost.')) {
      return;
    }

    setIsRestoring(snapshotId);
    setError(null);

    try {
      await invoke('restore_snapshot', {
        id: vmId,
        snapshotId,
      });
      
      // Reload snapshots to get updated state
      await loadSnapshots();
    } catch (err) {
      setError(`Failed to restore snapshot: ${err}`);
    } finally {
      setIsRestoring(null);
    }
  };

  const formatDate = (dateString: string) => {
    const date = new Date(dateString);
    return date.toLocaleString();
  };

  const formatSize = (sizeMb: number) => {
    if (sizeMb < 1024) {
      return `${sizeMb} MB`;
    } else {
      return `${(sizeMb / 1024).toFixed(1)} GB`;
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500"></div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex justify-between items-center">
        <h2 className="text-2xl font-bold text-gray-900">Snapshots - {vmName}</h2>
        <button
          onClick={() => setShowCreateForm(!showCreateForm)}
          className="flex items-center px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors"
        >
          <PhotoIcon className="h-5 w-5 mr-2" />
          Create Snapshot
        </button>
      </div>

      {/* Error Display */}
      {error && (
        <div className="bg-red-50 border border-red-200 rounded-md p-4">
          <div className="flex items-center">
            <ExclamationTriangleIcon className="h-5 w-5 text-red-500 mr-2" />
            <div className="text-red-800">{error}</div>
          </div>
        </div>
      )}

      {/* Create Snapshot Form */}
      {showCreateForm && (
        <div className="bg-gray-50 border border-gray-200 rounded-lg p-6">
          <h3 className="text-lg font-medium text-gray-900 mb-4">Create New Snapshot</h3>
          
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Snapshot Name
              </label>
              <input
                type="text"
                value={newSnapshotName}
                onChange={(e) => setNewSnapshotName(e.target.value)}
                className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                placeholder="Enter snapshot name"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Description
              </label>
              <textarea
                value={newSnapshotDescription}
                onChange={(e) => setNewSnapshotDescription(e.target.value)}
                rows={3}
                className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                placeholder="Enter snapshot description (optional)"
              />
            </div>

            <div className="flex justify-end space-x-2">
              <button
                onClick={() => setShowCreateForm(false)}
                className="px-4 py-2 border border-gray-300 rounded-md text-gray-700 hover:bg-gray-50"
              >
                Cancel
              </button>
              <button
                onClick={createSnapshot}
                disabled={isCreating || !newSnapshotName.trim()}
                className={`px-4 py-2 rounded-md text-sm font-medium ${
                  isCreating || !newSnapshotName.trim()
                    ? 'bg-gray-100 text-gray-400 cursor-not-allowed'
                    : 'bg-blue-600 text-white hover:bg-blue-700'
                }`}
              >
                {isCreating ? 'Creating...' : 'Create Snapshot'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Snapshots List */}
      <div className="bg-white shadow rounded-lg overflow-hidden">
        {snapshots.length === 0 ? (
          <div className="p-8 text-center">
            <PhotoIcon className="h-12 w-12 text-gray-400 mx-auto mb-4" />
            <h3 className="text-lg font-medium text-gray-900 mb-2">No snapshots found</h3>
            <p className="text-gray-500">
              Create a snapshot to save the current state of your virtual machine.
            </p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="min-w-full divide-y divide-gray-200">
              <thead className="bg-gray-50">
                <tr>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Name
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Description
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Created
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Size
                  </th>
                  <th className="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Actions
                  </th>
                </tr>
              </thead>
              <tbody className="bg-white divide-y divide-gray-200">
                {snapshots.map((snapshot) => (
                  <tr key={snapshot.id} className="hover:bg-gray-50">
                    <td className="px-6 py-4 whitespace-nowrap">
                      <div className="flex items-center">
                        <DocumentTextIcon className="h-5 w-5 text-gray-400 mr-2" />
                        <div className="text-sm font-medium text-gray-900">
                          {snapshot.name}
                        </div>
                      </div>
                    </td>
                    <td className="px-6 py-4">
                      <div className="text-sm text-gray-500 max-w-xs truncate">
                        {snapshot.description || 'No description'}
                      </div>
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap">
                      <div className="flex items-center text-sm text-gray-500">
                        <ClockIcon className="h-4 w-4 mr-1" />
                        {formatDate(snapshot.created_at)}
                      </div>
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap">
                      <div className="text-sm text-gray-500">
                        {formatSize(snapshot.size_mb)}
                      </div>
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-right text-sm font-medium">
                      <button
                        onClick={() => restoreSnapshot(snapshot.id)}
                        disabled={isRestoring === snapshot.id}
                        className={`inline-flex items-center px-3 py-1.5 rounded-md text-xs font-medium ${
                          isRestoring === snapshot.id
                            ? 'bg-gray-100 text-gray-400 cursor-not-allowed'
                            : 'text-blue-600 hover:text-blue-900'
                        }`}
                      >
                        {isRestoring === snapshot.id ? (
                          <>
                            <div className="animate-spin rounded-full h-3 w-3 border-b border-blue-600 mr-1"></div>
                            Restoring...
                          </>
                        ) : (
                          <>
                            <ArrowDownTrayIcon className="h-3 w-3 mr-1" />
                            Restore
                          </>
                        )}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Usage Information */}
      <div className="bg-blue-50 border border-blue-200 rounded-lg p-6">
        <h3 className="text-lg font-medium text-blue-900 mb-4">About Snapshots</h3>
        <div className="space-y-2 text-sm text-blue-800">
          <p>
            <strong>What are snapshots?</strong> Snapshots capture the complete state of your VM at a specific point in time, including memory, CPU state, and storage.
          </p>
          <p>
            <strong>When to use snapshots:</strong>
          </p>
          <ul className="ml-4 list-disc space-y-1">
            <li>Before making major changes to your system</li>
            <li>Before installing new software or updates</li>
            <li>To create restore points for testing</li>
            <li>To save specific configurations you want to reuse</li>
          </ul>
          <p>
            <strong>Storage:</strong> Snapshots are stored efficiently using delta compression, so multiple snapshots don't use excessive disk space.
          </p>
        </div>
      </div>
    </div>
  );
};