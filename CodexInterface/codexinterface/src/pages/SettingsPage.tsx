import React from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card'
import { Input } from '../components/ui/input'
import { Button } from '../components/ui/button'
import { Switch } from '../components/ui/switch'
import { Label } from '../components/ui/label'
import { Key, HardDrive, Mic, Wrench, Activity } from 'lucide-react'

const SettingsPage: React.FC = () => {
  return (
    <div className="grid grid-cols-12 gap-3 h-[calc(100vh-4rem-3rem)]">
      {/* Top row */}
      <div className="col-span-6">
        {/* LLM API Keys */}
        <Card className="border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center gap-2">
              <Key size={18} className="text-primary/80" />
              <CardTitle className="text-primary-foreground">LLM API Keys</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4 pt-2 relative z-10">
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="openai-api-key">OpenAI API Key</Label>
                <div className="flex gap-2">
                  <Input
                    id="openai-api-key"
                    type="password"
                    placeholder="Enter your OpenAI API key"
                    className="bg-zinc-800/60 border-zinc-700 text-zinc-200"
                  />
                  <Button className="bg-primary/80 hover:bg-primary/90">Save</Button>
                </div>
              </div>
              <div className="space-y-2">
                <Label htmlFor="anthropic-api-key">Anthropic API Key</Label>
                <div className="flex gap-2">
                  <Input
                    id="anthropic-api-key"
                    type="password"
                    placeholder="Enter your Anthropic API key"
                    className="bg-zinc-800/60 border-zinc-700 text-zinc-200"
                  />
                  <Button className="bg-primary/80 hover:bg-primary/90">Save</Button>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
      
      <div className="col-span-6">
        {/* File System Settings */}
        <Card className="border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center gap-2">
              <HardDrive size={18} className="text-primary/80" />
              <CardTitle className="text-primary-foreground">File System Settings</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4 pt-2 relative z-10">
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="storage-path">Default Storage Path</Label>
                <div className="flex gap-2">
                  <Input
                    id="storage-path"
                    placeholder="/path/to/storage"
                    className="bg-zinc-800/60 border-zinc-700 text-zinc-200"
                  />
                  <Button className="bg-zinc-700/80 hover:bg-zinc-700">Browse</Button>
                </div>
              </div>
              
              <div className="flex items-center space-x-2">
                <Switch id="auto-backup" />
                <Label htmlFor="auto-backup">Enable automatic backups</Label>
              </div>
              
              <div className="flex items-center space-x-2">
                <Switch id="compression" />
                <Label htmlFor="compression">Use compression for stored files</Label>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
      
      {/* Middle row */}
      <div className="col-span-6">
        {/* Whisper Settings */}
        <Card className="border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center gap-2">
              <Mic size={18} className="text-primary/80" />
              <CardTitle className="text-primary-foreground">Whisper Settings</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4 pt-2 relative z-10">
            <div className="space-y-3">
              <div className="flex items-center space-x-2">
                <Switch id="use-whisper-local" />
                <Label htmlFor="use-whisper-local">Use local Whisper model</Label>
              </div>
              
              <div className="space-y-2">
                <Label htmlFor="whisper-model">Whisper Model</Label>
                <div className="flex gap-2 items-center">
                  <select 
                    id="whisper-model"
                    className="w-full h-10 rounded-md border border-zinc-700 bg-zinc-800/60 px-3 py-2 text-sm text-zinc-200"
                  >
                    <option value="tiny">Tiny (fast, less accurate)</option>
                    <option value="base">Base</option>
                    <option value="small">Small</option>
                    <option value="medium">Medium</option>
                    <option value="large">Large (slow, most accurate)</option>
                  </select>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
        
        {/* Tool API Keys */}
        <Card className="mt-3 border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center gap-2">
              <Wrench size={18} className="text-primary/80" />
              <CardTitle className="text-primary-foreground">Tool API Keys</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4 pt-2 relative z-10">
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="serper-api-key">Serper API Key</Label>
                <div className="flex gap-2">
                  <Input
                    id="serper-api-key"
                    type="password"
                    placeholder="Enter your Serper API key"
                    className="bg-zinc-800/60 border-zinc-700 text-zinc-200"
                  />
                  <Button className="bg-primary/80 hover:bg-primary/90 whitespace-nowrap">Save</Button>
                </div>
              </div>
              
              <div className="space-y-2">
                <Label htmlFor="firecrawl-api-key">Firecrawl API Key</Label>
                <div className="flex gap-2">
                  <Input
                    id="firecrawl-api-key"
                    type="password"
                    placeholder="Enter your Firecrawl API key"
                    className="bg-zinc-800/60 border-zinc-700 text-zinc-200"
                  />
                  <Button className="bg-primary/80 hover:bg-primary/90 whitespace-nowrap">Save</Button>
                </div>
              </div>
              
              <div className="space-y-2">
                <Label htmlFor="github-api-key">GitHub API Token</Label>
                <div className="flex gap-2">
                  <Input
                    id="github-api-key"
                    type="password"
                    placeholder="Enter your GitHub token"
                    className="bg-zinc-800/60 border-zinc-700 text-zinc-200"
                  />
                  <Button className="bg-primary/80 hover:bg-primary/90 whitespace-nowrap">Save</Button>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
      
      <div className="col-span-6">
        {/* System Status */}
        <Card className="h-full border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center gap-2">
              <Activity size={18} className="text-primary/80" />
              <CardTitle className="text-primary-foreground">System Status</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4 pt-2 relative z-10">
            <div className="space-y-5">
              <div className="space-y-2">
                <div className="flex justify-between items-center">
                  <Label>Memory Usage</Label>
                  <span className="text-sm text-zinc-300">4.2 GB / 16 GB</span>
                </div>
                <div className="h-2 w-full bg-zinc-700/50 rounded-full">
                  <div className="h-2 bg-emerald-500/70 rounded-full" style={{ width: '26%' }}></div>
                </div>
              </div>
              
              <div className="space-y-2">
                <div className="flex justify-between items-center">
                  <Label>CPU Usage</Label>
                  <span className="text-sm text-zinc-300">35%</span>
                </div>
                <div className="h-2 w-full bg-zinc-700/50 rounded-full">
                  <div className="h-2 bg-amber-500/70 rounded-full" style={{ width: '35%' }}></div>
                </div>
              </div>
              
              <div className="space-y-2">
                <div className="flex justify-between items-center">
                  <Label>Storage</Label>
                  <span className="text-sm text-zinc-300">128 GB / 512 GB</span>
                </div>
                <div className="h-2 w-full bg-zinc-700/50 rounded-full">
                  <div className="h-2 bg-blue-500/70 rounded-full" style={{ width: '25%' }}></div>
                </div>
              </div>
              
              <div className="space-y-1">
                <Label>System Information</Label>
                <div className="bg-zinc-800/80 border border-zinc-700/50 rounded-md p-3 text-sm text-zinc-300">
                  <div className="grid grid-cols-2 gap-y-2">
                    <span className="text-zinc-400">OS:</span>
                    <span>Windows 11 Pro</span>
                    <span className="text-zinc-400">CPU:</span>
                    <span>Intel Core i9-12900K</span>
                    <span className="text-zinc-400">GPU:</span>
                    <span>NVIDIA RTX 4080</span>
                    <span className="text-zinc-400">RAM:</span>
                    <span>32 GB DDR5</span>
                    <span className="text-zinc-400">App Version:</span>
                    <span>1.3.5</span>
                  </div>
                </div>
              </div>
              
              <div className="flex justify-end">
                <Button className="bg-zinc-700/80 hover:bg-zinc-700">
                  Generate System Report
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

export default SettingsPage
