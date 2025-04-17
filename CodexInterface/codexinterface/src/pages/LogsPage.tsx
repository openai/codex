import React, { useState } from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card'
import { Button } from '../components/ui/button'
import { Input } from '../components/ui/input'
import { FileText, BarChart2, Filter, ArrowUpDown } from 'lucide-react'

// Type definitions
interface LogEntry {
  id: string;
  timestamp: Date;
  level: 'info' | 'warning' | 'error' | 'debug';
  source: string;
  message: string;
}

const LogsPage: React.FC = () => {
  // Sample log entries
  const [logs, setLogs] = useState<LogEntry[]>([
    {
      id: '1',
      timestamp: new Date(Date.now() - 1000 * 60 * 5),
      level: 'info',
      source: 'System',
      message: 'Application started successfully'
    },
    {
      id: '2',
      timestamp: new Date(Date.now() - 1000 * 60 * 4),
      level: 'debug',
      source: 'FileSystem',
      message: 'Loading configuration from settings.json'
    },
    {
      id: '3',
      timestamp: new Date(Date.now() - 1000 * 60 * 3),
      level: 'warning',
      source: 'Network',
      message: 'API response time exceeded threshold: 1500ms'
    },
    {
      id: '4',
      timestamp: new Date(Date.now() - 1000 * 60 * 2),
      level: 'error',
      source: 'Database',
      message: 'Failed to connect to database at localhost:5432'
    },
    {
      id: '5',
      timestamp: new Date(Date.now() - 1000 * 60 * 1),
      level: 'info',
      source: 'Auth',
      message: 'User session started'
    }
  ]);
  
  // Filter state
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedLevel, setSelectedLevel] = useState<string>('all');
  
  // Function to get log level style
  const getLevelStyle = (level: string) => {
    switch(level) {
      case 'info':
        return 'bg-blue-500/20 text-blue-300 border-blue-500/30';
      case 'warning':
        return 'bg-amber-500/20 text-amber-300 border-amber-500/30';
      case 'error':
        return 'bg-red-500/20 text-red-300 border-red-500/30';
      case 'debug':
        return 'bg-gray-500/20 text-gray-300 border-gray-500/30';
      default:
        return 'bg-zinc-500/20 text-zinc-300 border-zinc-500/30';
    }
  };
  
  // Format timestamp
  const formatTime = (date: Date) => {
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  };
  
  // Format date
  const formatDate = (date: Date) => {
    return date.toLocaleDateString();
  };
  
  // Filter logs based on search and level
  const filteredLogs = logs.filter(log => {
    const matchesSearch = searchQuery === '' || 
      log.message.toLowerCase().includes(searchQuery.toLowerCase()) ||
      log.source.toLowerCase().includes(searchQuery.toLowerCase());
    
    const matchesLevel = selectedLevel === 'all' || log.level === selectedLevel;
    
    return matchesSearch && matchesLevel;
  });

  return (
    <div className="grid grid-cols-12 gap-3 h-[calc(100vh-4rem-3rem)]">
      {/* Left column - Filters and controls */}
      <div className="col-span-3 flex flex-col gap-3">
        {/* Search and filters */}
        <Card className="border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center gap-2">
              <Filter size={18} className="text-primary/80" />
              <CardTitle className="text-primary-foreground">Log Filters</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4 pt-2 relative z-10">
            <div className="space-y-4">
              <div>
                <label className="text-sm text-zinc-400 mb-1 block">Search Logs</label>
                <Input
                  placeholder="Search by message or source..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="bg-zinc-800/60 border-zinc-700 text-zinc-200"
                />
              </div>
              
              <div>
                <label className="text-sm text-zinc-400 mb-1 block">Log Level</label>
                <select 
                  value={selectedLevel}
                  onChange={(e) => setSelectedLevel(e.target.value)}
                  className="w-full h-10 rounded-md border border-zinc-700 bg-zinc-800/60 px-3 py-2 text-sm text-zinc-200"
                >
                  <option value="all">All Levels</option>
                  <option value="info">Info</option>
                  <option value="warning">Warning</option>
                  <option value="error">Error</option>
                  <option value="debug">Debug</option>
                </select>
              </div>
              
              <div>
                <label className="text-sm text-zinc-400 mb-1 block">Time Range</label>
                <select 
                  className="w-full h-10 rounded-md border border-zinc-700 bg-zinc-800/60 px-3 py-2 text-sm text-zinc-200"
                >
                  <option value="1h">Last Hour</option>
                  <option value="24h">Last 24 Hours</option>
                  <option value="7d">Last 7 Days</option>
                  <option value="30d">Last 30 Days</option>
                  <option value="custom">Custom Range</option>
                </select>
              </div>
              
              <Button className="w-full bg-primary/80 hover:bg-primary/90">
                Apply Filters
              </Button>
            </div>
          </CardContent>
        </Card>
        
        {/* Log statistics */}
        <Card className="flex-grow border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center gap-2">
              <BarChart2 size={18} className="text-primary/80" />
              <CardTitle className="text-primary-foreground">Log Statistics</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4 pt-2 relative z-10">
            <div className="space-y-3">
              <div className="flex justify-between items-center">
                <span className="text-sm text-zinc-400">Total Entries</span>
                <span className="text-zinc-200 font-medium">{logs.length}</span>
              </div>
              
              <div className="flex justify-between items-center">
                <span className="text-sm text-zinc-400">Info Logs</span>
                <span className="text-blue-300 font-medium">{logs.filter(log => log.level === 'info').length}</span>
              </div>
              
              <div className="flex justify-between items-center">
                <span className="text-sm text-zinc-400">Warnings</span>
                <span className="text-amber-300 font-medium">{logs.filter(log => log.level === 'warning').length}</span>
              </div>
              
              <div className="flex justify-between items-center">
                <span className="text-sm text-zinc-400">Errors</span>
                <span className="text-red-300 font-medium">{logs.filter(log => log.level === 'error').length}</span>
              </div>
              
              <div className="flex justify-between items-center">
                <span className="text-sm text-zinc-400">Debug Logs</span>
                <span className="text-gray-300 font-medium">{logs.filter(log => log.level === 'debug').length}</span>
              </div>
              
              <div className="h-40 flex items-center justify-center bg-zinc-800/60 border border-zinc-700/50 rounded-md mt-4">
                <span className="text-zinc-500 text-sm">[Log Distribution Chart]</span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
      
      {/* Main log display */}
      <div className="col-span-9">
        <Card className="h-full border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <FileText size={18} className="text-primary/80" />
                <CardTitle className="text-primary-foreground">Log Entries</CardTitle>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-sm text-zinc-400">Showing {filteredLogs.length} of {logs.length} entries</span>
                <Button variant="outline" size="sm" className="h-8 bg-zinc-800/80 border-zinc-700">
                  <ArrowUpDown size={14} className="mr-1" />
                  <span>Sort</span>
                </Button>
                <Button variant="outline" size="sm" className="h-8 bg-zinc-800/80 border-zinc-700">Export</Button>
              </div>
            </div>
          </CardHeader>
          <CardContent className="p-0 relative z-10 h-[calc(100%-4rem)] overflow-auto">
            <div className="min-w-full">
              <div className="grid grid-cols-12 py-2 px-4 bg-zinc-800/80 border-b border-zinc-700/60 text-xs font-medium text-zinc-400 uppercase tracking-wider">
                <div className="col-span-2">Timestamp</div>
                <div className="col-span-1">Level</div>
                <div className="col-span-2">Source</div>
                <div className="col-span-7">Message</div>
              </div>
              
              <div className="divide-y divide-zinc-800">
                {filteredLogs.map((log) => (
                  <div key={log.id} className="grid grid-cols-12 py-3 px-4 hover:bg-zinc-800/30">
                    <div className="col-span-2 text-sm text-zinc-400">
                      <div className="flex flex-col">
                        <span>{formatDate(log.timestamp)}</span>
                        <span>{formatTime(log.timestamp)}</span>
                      </div>
                    </div>
                    <div className="col-span-1">
                      <span className={`px-2 py-1 rounded-md text-xs font-medium ${getLevelStyle(log.level)}`}>
                        {log.level}
                      </span>
                    </div>
                    <div className="col-span-2 text-sm text-zinc-300">{log.source}</div>
                    <div className="col-span-7 text-sm text-zinc-300">{log.message}</div>
                  </div>
                ))}
              </div>
              
              {filteredLogs.length === 0 && (
                <div className="flex items-center justify-center h-40">
                  <span className="text-zinc-500">No logs match your current filters</span>
                </div>
              )}
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

export default LogsPage
