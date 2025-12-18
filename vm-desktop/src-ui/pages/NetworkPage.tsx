import React, { useState } from 'react';
import { Card, Button, Badge } from '../atoms';

interface NetworkInterface {
  id: string;
  name: string;
  type: 'Bridge' | 'NAT' | 'Host-only';
  status: 'Connected' | 'Disconnected';
  ipAddress: string;
  macAddress: string;
  bandwidth: string;
  transmitted: string;
  received: string;
}

interface NetworkRule {
  id: string;
  name: string;
  type: 'Port Forward' | 'VLAN' | 'QoS';
  source: string;
  destination: string;
  enabled: boolean;
}

export const NetworkPage: React.FC = () => {
  const [interfaces, setInterfaces] = useState<NetworkInterface[]>([
    {
      id: 'eth0',
      name: 'eth0',
      type: 'Bridge',
      status: 'Connected',
      ipAddress: '192.168.1.100',
      macAddress: '08:00:27:2A:5C:D3',
      bandwidth: '1000 Mbps',
      transmitted: '2.3 GB',
      received: '5.8 GB',
    },
    {
      id: 'eth1',
      name: 'eth1',
      type: 'NAT',
      status: 'Connected',
      ipAddress: '10.0.2.15',
      macAddress: '08:00:27:3E:1F:92',
      bandwidth: '100 Mbps',
      transmitted: '120 MB',
      received: '456 MB',
    },
    {
      id: 'eth2',
      name: 'eth2',
      type: 'Host-only',
      status: 'Disconnected',
      ipAddress: '192.168.56.1',
      macAddress: '08:00:27:4B:7C:A9',
      bandwidth: '0 Mbps',
      transmitted: '0 B',
      received: '0 B',
    },
  ]);

  const [rules, setRules] = useState<NetworkRule[]>([
    {
      id: 'r1',
      name: 'SSH Access',
      type: 'Port Forward',
      source: '127.0.0.1:2222',
      destination: '192.168.1.100:22',
      enabled: true,
    },
    {
      id: 'r2',
      name: 'HTTP Server',
      type: 'Port Forward',
      source: '127.0.0.1:8080',
      destination: '192.168.1.100:80',
      enabled: true,
    },
    {
      id: 'r3',
      name: 'HTTPS Server',
      type: 'Port Forward',
      source: '127.0.0.1:8443',
      destination: '192.168.1.100:443',
      enabled: false,
    },
    {
      id: 'r4',
      name: 'VLAN Isolation',
      type: 'VLAN',
      source: 'eth0',
      destination: 'VLAN 100',
      enabled: true,
    },
  ]);

  const [selectedInterfaceId, setSelectedInterfaceId] = useState('eth0');
  const [selectedRuleId, setSelectedRuleId] = useState('r1');
  const [showAddRule, setShowAddRule] = useState(false);
  const [newRuleName, setNewRuleName] = useState('');
  const [newRuleType, setNewRuleType] = useState<'Port Forward' | 'VLAN' | 'QoS'>('Port Forward');
  const [newRuleSource, setNewRuleSource] = useState('');
  const [newRuleDestination, setNewRuleDestination] = useState('');

  const selectedInterface = interfaces.find(i => i.id === selectedInterfaceId);
  const selectedRule = rules.find(r => r.id === selectedRuleId);

  const handleToggleRule = (ruleId: string) => {
    setRules(rules.map(r => r.id === ruleId ? { ...r, enabled: !r.enabled } : r));
  };

  const handleAddRule = () => {
    if (!newRuleName.trim() || !newRuleSource.trim() || !newRuleDestination.trim()) return;

    const newRule: NetworkRule = {
      id: `r${rules.length + 1}`,
      name: newRuleName,
      type: newRuleType,
      source: newRuleSource,
      destination: newRuleDestination,
      enabled: true,
    };

    setRules([...rules, newRule]);
    setNewRuleName('');
    setNewRuleSource('');
    setNewRuleDestination('');
    setShowAddRule(false);
  };

  const handleDeleteRule = (ruleId: string) => {
    if (window.confirm('Delete this rule?')) {
      setRules(rules.filter(r => r.id !== ruleId));
      setSelectedRuleId(null);
    }
  };

  const handleEnableInterface = (interfaceId: string) => {
    setInterfaces(interfaces.map(i =>
      i.id === interfaceId
        ? { ...i, status: i.status === 'Connected' ? 'Disconnected' : 'Connected' }
        : i
    ));
  };

  return (
    <div className="space-y-4 p-4">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold text-gray-800">Network Configuration</h1>
        <p className="text-gray-500">Manage network interfaces and routing rules</p>
      </div>

      {/* Main Content */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Interfaces Panel */}
        <div className="lg:col-span-2 space-y-4">
          {/* Network Interfaces */}
          <Card className="p-4">
            <h2 className="font-semibold text-gray-800 mb-3">Network Interfaces</h2>
            <div className="space-y-2">
              {interfaces.map(iface => (
                <div
                  key={iface.id}
                  onClick={() => setSelectedInterfaceId(iface.id)}
                  className={`p-3 rounded-lg cursor-pointer transition border-2 ${
                    selectedInterfaceId === iface.id
                      ? 'bg-blue-100 border-blue-500'
                      : 'bg-gray-50 hover:bg-gray-100 border-gray-200'
                  }`}
                >
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="flex items-center gap-2">
                        <h3 className="font-semibold text-gray-800">{iface.name}</h3>
                        <Badge className={`text-xs ${
                          iface.status === 'Connected'
                            ? 'bg-green-100 text-green-800'
                            : 'bg-red-100 text-red-800'
                        }`}>
                          {iface.status}
                        </Badge>
                      </div>
                      <p className="text-sm text-gray-600">{iface.type}</p>
                      <div className="grid grid-cols-3 gap-4 mt-2 text-xs text-gray-500">
                        <div>
                          <span className="font-semibold">{iface.ipAddress}</span>
                          <p className="text-gray-400">IP Address</p>
                        </div>
                        <div>
                          <span className="font-semibold">{iface.bandwidth}</span>
                          <p className="text-gray-400">Speed</p>
                        </div>
                        <div>
                          <span className="font-semibold">{iface.received}</span>
                          <p className="text-gray-400">Received</p>
                        </div>
                      </div>
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleEnableInterface(iface.id);
                      }}
                      className={`px-3 py-1 rounded text-sm font-semibold transition ${
                        iface.status === 'Connected'
                          ? 'bg-red-500 hover:bg-red-600 text-white'
                          : 'bg-green-500 hover:bg-green-600 text-white'
                      }`}
                    >
                      {iface.status === 'Connected' ? 'Disable' : 'Enable'}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </Card>

          {/* Routing Rules */}
          <Card className="p-4">
            <div className="flex items-center justify-between mb-3">
              <h2 className="font-semibold text-gray-800">Routing Rules</h2>
              <Button
                onClick={() => setShowAddRule(!showAddRule)}
                className="text-sm bg-blue-500 hover:bg-blue-600"
              >
                {showAddRule ? '✕' : '➕ Add Rule'}
              </Button>
            </div>

            {showAddRule && (
              <div className="bg-blue-50 p-3 rounded-lg mb-3 space-y-2 border-2 border-blue-300">
                <input
                  type="text"
                  value={newRuleName}
                  onChange={(e) => setNewRuleName(e.target.value)}
                  placeholder="Rule name"
                  className="w-full px-3 py-2 border rounded text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
                <select
                  value={newRuleType}
                  onChange={(e) => setNewRuleType(e.target.value as any)}
                  className="w-full px-3 py-2 border rounded text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                >
                  <option value="Port Forward">Port Forward</option>
                  <option value="VLAN">VLAN</option>
                  <option value="QoS">QoS</option>
                </select>
                <input
                  type="text"
                  value={newRuleSource}
                  onChange={(e) => setNewRuleSource(e.target.value)}
                  placeholder="Source (e.g., 127.0.0.1:8080)"
                  className="w-full px-3 py-2 border rounded text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
                <input
                  type="text"
                  value={newRuleDestination}
                  onChange={(e) => setNewRuleDestination(e.target.value)}
                  placeholder="Destination (e.g., 192.168.1.100:80)"
                  className="w-full px-3 py-2 border rounded text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
                <div className="flex gap-2">
                  <Button
                    onClick={handleAddRule}
                    className="flex-1 bg-green-500 hover:bg-green-600 text-sm"
                  >
                    Add
                  </Button>
                  <Button
                    onClick={() => setShowAddRule(false)}
                    className="flex-1 bg-gray-400 hover:bg-gray-500 text-sm"
                  >
                    Cancel
                  </Button>
                </div>
              </div>
            )}

            <div className="space-y-2">
              {rules.map(rule => (
                <div
                  key={rule.id}
                  onClick={() => setSelectedRuleId(rule.id)}
                  className={`p-3 rounded-lg cursor-pointer transition border-2 ${
                    selectedRuleId === rule.id
                      ? 'bg-blue-100 border-blue-500'
                      : 'bg-gray-50 hover:bg-gray-100 border-gray-200'
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <div className="flex-1">
                      <div className="flex items-center gap-2">
                        <h4 className="font-semibold text-gray-800">{rule.name}</h4>
                        <Badge className="text-xs bg-purple-100 text-purple-800">
                          {rule.type}
                        </Badge>
                      </div>
                      <div className="text-xs text-gray-600 mt-1">
                        {rule.source} → {rule.destination}
                      </div>
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleToggleRule(rule.id);
                      }}
                      className={`px-2 py-1 rounded text-xs font-semibold transition ${
                        rule.enabled
                          ? 'bg-green-500 hover:bg-green-600 text-white'
                          : 'bg-gray-400 hover:bg-gray-500 text-white'
                      }`}
                    >
                      {rule.enabled ? '✓ On' : '✕ Off'}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </Card>
        </div>

        {/* Right Panel: Details */}
        <div className="space-y-4">
          {/* Interface Details */}
          {selectedInterface && (
            <Card className="p-4">
              <h2 className="font-semibold text-gray-800 mb-3">Interface Details</h2>
              <div className="space-y-3 text-sm">
                <div>
                  <p className="text-gray-500">Interface</p>
                  <p className="font-semibold text-gray-800">{selectedInterface.name}</p>
                </div>
                <div>
                  <p className="text-gray-500">Type</p>
                  <p className="font-semibold text-gray-800">{selectedInterface.type}</p>
                </div>
                <div>
                  <p className="text-gray-500">Status</p>
                  <p className={`font-semibold ${
                    selectedInterface.status === 'Connected' ? 'text-green-600' : 'text-red-600'
                  }`}>
                    {selectedInterface.status}
                  </p>
                </div>
                <div>
                  <p className="text-gray-500">IP Address</p>
                  <p className="font-semibold text-gray-800 break-words">{selectedInterface.ipAddress}</p>
                </div>
                <div>
                  <p className="text-gray-500">MAC Address</p>
                  <p className="font-semibold text-gray-800 font-mono text-xs">{selectedInterface.macAddress}</p>
                </div>
                <div className="pt-3 border-t">
                  <p className="text-gray-500 text-xs mb-2">Traffic Statistics</p>
                  <div className="space-y-1">
                    <div className="flex justify-between">
                      <span className="text-gray-600">Sent:</span>
                      <span className="font-semibold">{selectedInterface.transmitted}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-gray-600">Received:</span>
                      <span className="font-semibold">{selectedInterface.received}</span>
                    </div>
                  </div>
                </div>
              </div>
            </Card>
          )}

          {/* Rule Details */}
          {selectedRule && (
            <Card className="p-4">
              <h2 className="font-semibold text-gray-800 mb-3">Rule Details</h2>
              <div className="space-y-3 text-sm">
                <div>
                  <p className="text-gray-500">Name</p>
                  <p className="font-semibold text-gray-800">{selectedRule.name}</p>
                </div>
                <div>
                  <p className="text-gray-500">Type</p>
                  <p className="font-semibold text-gray-800">{selectedRule.type}</p>
                </div>
                <div>
                  <p className="text-gray-500">Source</p>
                  <p className="font-semibold text-gray-800 break-words">{selectedRule.source}</p>
                </div>
                <div>
                  <p className="text-gray-500">Destination</p>
                  <p className="font-semibold text-gray-800 break-words">{selectedRule.destination}</p>
                </div>
                <div>
                  <p className="text-gray-500">Status</p>
                  <p className={`font-semibold ${selectedRule.enabled ? 'text-green-600' : 'text-red-600'}`}>
                    {selectedRule.enabled ? 'Enabled' : 'Disabled'}
                  </p>
                </div>

                <div className="pt-3 border-t space-y-2">
                  <Button
                    onClick={() => handleToggleRule(selectedRule.id)}
                    className={`w-full ${selectedRule.enabled ? 'bg-red-500 hover:bg-red-600' : 'bg-green-500 hover:bg-green-600'}`}
                  >
                    {selectedRule.enabled ? 'Disable' : 'Enable'}
                  </Button>
                  <Button
                    onClick={() => handleDeleteRule(selectedRule.id)}
                    className="w-full bg-red-700 hover:bg-red-800"
                  >
                    🗑️ Delete
                  </Button>
                </div>
              </div>
            </Card>
          )}

          {/* Network Summary */}
          <Card className="p-4">
            <h2 className="font-semibold text-gray-800 mb-3">Summary</h2>
            <div className="space-y-2 text-sm">
              <div className="flex justify-between">
                <span className="text-gray-600">Active Interfaces:</span>
                <span className="font-semibold">{interfaces.filter(i => i.status === 'Connected').length}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-600">Total Rules:</span>
                <span className="font-semibold">{rules.length}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-600">Enabled Rules:</span>
                <span className="font-semibold">{rules.filter(r => r.enabled).length}</span>
              </div>
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
};
