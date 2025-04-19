import React, { useState, useEffect } from 'react'
import { Button } from '../components/ui/button'
import { Input } from '../components/ui/input'
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card'
import { TreeView, TreeItem } from '../components/ui/tree-view'
import { Checkbox } from '../components/ui/checkbox'
import { Send, ArrowUp, ArrowRight, File, Folder, Plus, Paperclip, Loader2 } from 'lucide-react'

// Type definitions
interface TreeNode {
  id: string;
  label: string;
  type: 'file' | 'folder';
  children?: TreeNode[];
  defaultExpanded?: boolean;
}

interface Tool {
  id: number;
  name: string;
  checked: boolean;
}

interface TokenData {
  initialPrompt: string;
  outboundTokens: number;
  inboundTokens: number;
  cachedTokens: number;
  totalCacheAvailable: number;
  contextTokensUsed: number;
  totalContextTokens: number;
  outboundTokenCost: number;
  inboundTokenCost: number;
}

interface Message {
  id: number;
  content: string;
  isUser: boolean;
  timestamp: Date;
}

interface ActivityIndicatorStyle {
  outerBorder: {
    color: string;
    animate: 'pulse' | 'wave' | 'spin' | 'none';
  };
  middleBorder: {
    color: string;
    animate: 'pulse' | 'wave' | 'spin' | 'none';
  };
  center: {
    color: string;
    animate: 'pulse' | 'wave' | 'spin' | 'none';
  };
}

const CodexPage: React.FC = () => {
  const [prompt, setPrompt] = useState('')
  const [attachedFiles, setAttachedFiles] = useState<File[]>([])
  const fileInputRef = React.useRef<HTMLInputElement>(null)
  const [messages, setMessages] = useState<Message[]>([])
  const [loading, setLoading] = useState(false)
  // Tracks if the assistant has sent any content chunks yet
  const [assistantStarted, setAssistantStarted] = useState(false)
  
  // Directory Tree State (populated from server)
  // Directory Tree State (populated from server)
  const [directoryTree, setDirectoryTree] = useState<TreeNode[]>([])
  // Loading state for directory tree
  const [treeLoading, setTreeLoading] = useState(true)
  
  // Tools State
  const [tools, setTools] = useState<Tool[]>([
    { id: 1, name: 'Web Search', checked: true },
    { id: 2, name: 'Code Generation', checked: true },
    { id: 3, name: 'File Access', checked: false },
    { id: 4, name: 'Data Analysis', checked: true },
    { id: 5, name: 'Image Processing', checked: false }
  ])
  
  // Token usage data
  const [tokenData, setTokenData] = useState<TokenData>({
    initialPrompt: "Please rollback the current project to react 18",
    outboundTokens: 420,
    inboundTokens: 31800,
    cachedTokens: 88100,
    totalCacheAvailable: 1500000,
    contextTokensUsed: 84300,
    totalContextTokens: 200000,
    outboundTokenCost: 0.00001,
    inboundTokenCost: 0.00003
  })
  
  // Initialize conversation state and subscribe to server updates
  useEffect(() => {
    // Fetch initial state
    async function initialize() {
      try {
        const res = await fetch('/state')
        const data = await res.json()
        const items: Array<any> = data.items || []
        const initialMessages = items.map((item, index) => {
          const chunks = Array.isArray(item.content) ? item.content : []
          const content = chunks.map((c: any) => {
            const text = c.text || ''
            return c.type && c.type !== 'input_text'
              ? `[${c.type}] ${text}`
              : text
          }).join('')
          const isUser = item.role === 'user'
          return { id: index, content, isUser, timestamp: new Date() }
        })
        setMessages(initialMessages)
        // Set directory tree from server
        if (Array.isArray(data.tree)) {
        setDirectoryTree(data.tree)
        setTreeLoading(false)
        }
      } catch (err) {
        console.error('Failed to fetch initial state', err)
        setTreeLoading(false)
      }
    }
    initialize()
    // Connect to WebSocket for streaming updates
    const protocol = window.location.protocol === 'https:' ? 'wss' : 'ws'
    const socket = new WebSocket(`${protocol}://${window.location.host}/codex-ws`)
    socket.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data)
        // Handle incremental filesystem events
        if (msg.type === 'fs_event') {
          const { event: fsEvent, path: fsPath, nodeType } = msg as any
          // Apply filesystem event to directory tree
          setDirectoryTree(prev => {
            const clone = JSON.parse(JSON.stringify(prev)) as TreeNode[]
            const segments = fsPath.split('/')
            const parentSegments = segments.slice(0, -1)
            const parentId = parentSegments.join('/')
            const findNode = (nodes: TreeNode[], id: string): TreeNode | null => {
              for (const n of nodes) {
                if (n.id === id) return n
                if (n.children) {
                  const found = findNode(n.children, id)
                  if (found) return found
                }
              }
              return null
            }
            let siblings: TreeNode[] = clone
            if (parentId) {
              const parentNode = findNode(clone, parentId)
              if (!parentNode) return prev
              parentNode.children = parentNode.children || []
              siblings = parentNode.children
            }
            const name = segments[segments.length - 1]
            if (fsEvent === 'unlink') {
              const idx = siblings.findIndex(n => n.id === fsPath)
              if (idx !== -1) siblings.splice(idx, 1)
            } else if (fsEvent === 'add') {
              const idx = siblings.findIndex(n => n.id === fsPath)
              const newNode: TreeNode = { id: fsPath, label: name, type: nodeType, defaultExpanded: false }
              if (nodeType === 'folder') newNode.children = []
              if (idx !== -1) siblings[idx] = newNode
              else siblings.push(newNode)
            }
            return clone
          })
          return
        }
        // Handle loading indicator notifications
        if (msg.type === 'loading') {
          setLoading(msg.loading)
          // Reset assistant start on new loading cycle
          if (msg.loading) setAssistantStarted(false)
          // Toggle activity indicator color for run-level loading
          setActivityStyle(msg.loading ? loadingActivityStyle : defaultActivityStyle)
          return
        }
        if (msg.type === 'item' && msg.item) {
          const item = msg.item
          // Format content, including headers for non-text types
          const chunks = Array.isArray(item.content) ? item.content : []
          const formatted = chunks.map((c: any) => {
            const text = c.text || ''
            return c.type && c.type !== 'input_text'
              ? `[${c.type}] ${text}`
              : text
          }).join('')
          const isUser = item.role === 'user'
          // Mark assistant as started when receiving first assistant chunk
          if (!isUser && formatted && !assistantStarted) {
            setAssistantStarted(true)
          }
          const newId = typeof item.id === 'number' ? item.id : Date.now()
          setMessages(prev => {
            // Deduplicate user messages
            if (isUser) {
              const last = prev[prev.length - 1]
              if (last && last.isUser && last.content === formatted) {
                return prev
              }
              return [...prev, { id: newId, content: formatted, isUser, timestamp: new Date() }]
            }
            // If assistant streaming, merge into last message
            const last = prev[prev.length - 1]
            if (last && !last.isUser) {
              // Avoid duplicate identical chunks
              if (formatted && last.content.endsWith(formatted)) {
                return prev
              }
              const updated = [...prev]
              updated[updated.length - 1] = {
                ...last,
                content: last.content + formatted,
                timestamp: new Date(),
              }
              return updated
            }
            // Otherwise, start a new assistant message
            return [...prev, { id: newId, content: formatted, isUser, timestamp: new Date() }]
          })
        }
      } catch (e) {
        console.error('Error parsing WebSocket message', e)
      }
    }
    socket.onerror = (err) => console.error('WebSocket error', err)
    return () => { socket.close() }
  }, [])
  
  // Activity indicator style state
  // Default and loading styles for the activity indicator
  const defaultActivityStyle: ActivityIndicatorStyle = {
    outerBorder: { color: 'rgba(92,124,250,0.5)', animate: 'none' },
    middleBorder: { color: 'rgba(92,124,250,0.2)', animate: 'pulse' },
    center:       { color: 'rgb(92,124,250)', animate: 'none' }
  }
  const loadingActivityStyle: ActivityIndicatorStyle = {
    outerBorder: { color: 'rgba(248,113,113,0.5)', animate: 'spin' },
    middleBorder:{ color: 'rgba(248,113,113,0.7)', animate: 'pulse' },
    center:      { color: 'rgb(248,113,113)', animate: 'spin' }
  }
  const [activityStyle, setActivityStyle] = useState<ActivityIndicatorStyle>(defaultActivityStyle)
  
  // Calculate API cost
  const calculateApiCost = () => {
    return (
      (tokenData.outboundTokens * tokenData.outboundTokenCost) + 
      (tokenData.inboundTokens * tokenData.inboundTokenCost)
    ).toFixed(5)
  }
  
  // Calculate progress percentage
  const calculateContextPercentage = () => {
    return (tokenData.contextTokensUsed / tokenData.totalContextTokens) * 100
  }
  
  // Handle checkbox change
  const handleToolToggle = (id: number) => {
    setTools(tools.map(tool => 
      tool.id === id ? { ...tool, checked: !tool.checked } : tool
    ))
  }
  
  // Handle send message: append user message locally, show loading, then post prompt
  const handleSendMessage = async () => {
    if (!prompt.trim()) return
    // Append user message immediately
    const content = prompt
    const newId = Date.now()
    setMessages(prev => [...prev, { id: newId, content, isUser: true, timestamp: new Date() }])
    // Show loading spinner and red activity indicator
    setLoading(true)
    setActivityStyle(loadingActivityStyle)
    setAssistantStarted(false)
    // Send prompt to server
    try {
      await fetch('/prompt', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ prompt }),
      })
    } catch (err) {
      console.error('Error sending prompt', err)
    }
    setPrompt('')
  }
  
  // Recursive function to render tree nodes
  const renderTreeNodes = (nodes: TreeNode[]) => {
    return nodes.map(node => (
      <TreeItem 
        key={node.id} 
        label={node.label} 
        defaultExpanded={node.defaultExpanded}
        icon={node.type === 'folder' ? <Folder size={16} /> : <File size={16} />}
      >
        {node.children && node.children.length > 0 ? renderTreeNodes(node.children) : null}
      </TreeItem>
    ))
  }

  return (
    <div className="grid grid-cols-12 gap-6 h-[calc(100vh-4rem-3rem)]">
      {/* Left column - Activity, Tools, and Directory Tree */}
      <div className="col-span-4 flex flex-col gap-6 h-full">
        {/* Top row with Activity indicator and Tools/Knowledge */}
        <div className="flex h-32 space-x-2">
          {/* Activity indicator (left) */}
          <div className="flex-shrink-0 flex items-center justify-start w-24">
            <div 
              className={`w-24 h-24 rounded-full border-2 flex items-center justify-center shadow-lg relative before:absolute before:inset-0 before:rounded-full before:z-0`}
              style={{
                borderColor: activityStyle.outerBorder.color,
                backgroundColor: 'rgba(0,0,0,0.2)',
              }}
            >
              <div 
                className={`w-16 h-16 rounded-full flex items-center justify-center z-10 ${
                  activityStyle.middleBorder.animate === 'pulse' ? 'animate-pulse' : 
                  activityStyle.middleBorder.animate === 'spin' ? 'animate-spin' : 
                  activityStyle.middleBorder.animate === 'wave' ? 'animate-bounce' : ''
                }`}
                style={{ backgroundColor: activityStyle.middleBorder.color }}
              >
                <div 
                  className={`w-8 h-8 rounded-full shadow-lg ${
                    activityStyle.center.animate === 'pulse' ? 'animate-pulse' : 
                    activityStyle.center.animate === 'spin' ? 'animate-spin' : 
                    activityStyle.center.animate === 'wave' ? 'animate-bounce' : ''
                  }`}
                  style={{ backgroundColor: activityStyle.center.color }}
                ></div>
              </div>
            </div>
          </div>
          
          {/* Tools / Knowledge (right) */}
          <div className="flex-grow">
            <Card className="h-full border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
              <CardHeader className="p-3 pb-0 relative z-10">
                <CardTitle className="text-xl text-primary-foreground">Tools / Knowledge</CardTitle>
              </CardHeader>
              <CardContent className="p-3 pt-2 relative z-10">
                <div className="grid grid-cols-3 gap-2">
                  {tools.map((tool) => (
                    <div key={tool.id} className="flex items-center space-x-2">
                      <Checkbox 
                        id={`tool-${tool.id}`} 
                        checked={tool.checked}
                        onCheckedChange={() => handleToolToggle(tool.id)}
                        className="border-zinc-600 data-[state=checked]:bg-primary/80"
                      />
                      <label
                        htmlFor={`tool-${tool.id}`}
                        className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
                      >
                        {tool.name}
                      </label>
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          </div>
        </div>
        
        {/* Directory Tree */}
        <Card className="flex flex-col flex-grow border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden">
          <CardHeader className="p-4 flex-none z-10">
            <CardTitle className="text-primary-foreground">Directory Tree</CardTitle>
          </CardHeader>
          <CardContent className="p-4 pt-0 flex-grow overflow-auto">
            {treeLoading ? (
              <div className="h-full flex items-center justify-center">
                <Loader2 className="animate-spin h-8 w-8 text-zinc-500" />
              </div>
            ) : (
              <TreeView>
                {renderTreeNodes(directoryTree)}
              </TreeView>
            )}
          </CardContent>
        </Card>
      </div>
      
      {/* Right column - Full height chat interface */}
      <div className="col-span-8 flex flex-col h-full gap-6">
        {/* Initial Prompt, usage */}
        <Card className="h-auto border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-3 pb-1 relative z-10">
            <CardTitle className="text-primary-foreground text-lg">
              <span className="font-semibold">{tokenData.initialPrompt}</span>
            </CardTitle>
          </CardHeader>
          <CardContent className="p-3 relative z-10">
            <div className="space-y-1.5 text-sm">
              {/* Tokens */}
              <div className="flex items-center">
                <span className="w-36 text-zinc-400">Tokens:</span>
                <div className="flex items-center gap-1">
                  <ArrowUp className="text-green-400 h-3.5 w-3.5" />
                  <span className="text-green-400">{tokenData.outboundTokens.toLocaleString()}</span>
                  <ArrowRight className="h-3.5 w-3.5 text-zinc-500 mx-1" />
                  <span className="text-zinc-300">{tokenData.inboundTokens.toLocaleString()}</span>
                </div>
              </div>
              
              {/* Cache */}
              <div className="flex items-center">
                <span className="w-36 text-zinc-400">Cache:</span>
                <div className="flex items-center gap-1">
                  <ArrowUp className="text-green-400 h-3.5 w-3.5" />
                  <span className="text-green-400">{(tokenData.cachedTokens / 1000).toLocaleString()}k</span>
                  <ArrowRight className="h-3.5 w-3.5 text-zinc-500 mx-1" />
                  <span className="text-zinc-300">{(tokenData.totalCacheAvailable / 1000000).toLocaleString()}m</span>
                </div>
              </div>
              
              {/* Context Window */}
              <div className="flex items-center">
                <span className="w-36 text-zinc-400">Context Window:</span>
                <div className="flex items-center gap-2 flex-grow">
                  <span className="text-zinc-300">{(tokenData.contextTokensUsed / 1000).toLocaleString()}k</span>
                  <div className="h-1.5 bg-zinc-700/50 rounded-full flex-grow relative">
                    <div 
                      className="absolute h-full bg-primary/60 rounded-full"
                      style={{ width: `${calculateContextPercentage()}%` }}
                    ></div>
                  </div>
                  <span className="text-zinc-300">{(tokenData.totalContextTokens / 1000).toLocaleString()}k</span>
                </div>
              </div>
              
              {/* API Cost */}
              <div className="flex items-center">
                <span className="w-36 text-zinc-400">API Cost:</span>
                <span className="text-zinc-300">${calculateApiCost()}</span>
              </div>
            </div>
          </CardContent>
        </Card>
        
        {/* Chat Window - Takes most of the remaining height */}
        <Card className="flex-grow border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardContent className="h-full p-0 relative z-10">
            <div className="p-6 h-full overflow-y-auto space-y-4">
              {messages.map((message) => (
                <div key={message.id} className={`flex ${message.isUser ? 'justify-start' : 'justify-end'}`}>
                  <div className={`max-w-[75%] p-3 rounded-lg text-zinc-100 shadow-md ${
                    message.isUser 
                      ? 'bg-zinc-800/70 border border-zinc-700/30' 
                      : 'bg-primary/20 border border-primary/10'
                  }`}>
                    {message.content}
                  </div>
                </div>
              ))}
              {/* Loading indicator bubble when awaiting first assistant chunk */}
              {/* Show loading spinner until assistant chunks start */}
              {loading && !assistantStarted && (
                <div key="loading" className="flex justify-end">
                  <div className="max-w-[75%] p-3 rounded-lg text-zinc-100 shadow-md bg-primary/20 border border-primary/10 flex items-center">
                    <Loader2 className="animate-spin h-5 w-5 text-zinc-100" />
                  </div>
                </div>
              )}
            </div>
          </CardContent>
        </Card>
        
        {/* Prompt input and send button */}
        <div className="flex gap-4">
          <div className="flex-grow rounded-md bg-zinc-800/40 border border-zinc-700/50 shadow-[0_0_15px_rgba(0,0,0,0.5)] overflow-hidden relative">
            <div className="flex items-center h-full">
              <Input 
                className="h-11 border-none bg-transparent text-zinc-100 placeholder:text-zinc-500 focus-visible:ring-primary/30 px-3 flex-grow"
                placeholder="Prompt input" 
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
              />
              
              {/* File attachment indicator */}
              {attachedFiles.length > 0 && (
                <div 
                  className="flex items-center justify-center space-x-1 px-2 cursor-pointer"
                  onClick={() => setAttachedFiles([])}
                  title="Clear attached files"
                >
                  <Paperclip size={14} className="text-primary/70" />
                  <span className="text-xs text-primary/70">{attachedFiles.length}</span>
                </div>
              )}
              
              {/* Hidden file input */}
              <input
                type="file"
                multiple
                className="hidden"
                ref={fileInputRef}
                onChange={(e) => {
                  if (e.target.files && e.target.files.length > 0) {
                    const filesArray = Array.from(e.target.files);
                    setAttachedFiles(prev => [...prev, ...filesArray]);
                    e.target.value = ''; // Reset to allow selecting the same file again
                  }
                }}
              />
              
              {/* Add files button */}
              <div 
                className="w-7 h-7 rounded-full bg-zinc-700/50 hover:bg-zinc-700 flex items-center justify-center mr-2 cursor-pointer transition-colors"
                onClick={() => fileInputRef.current?.click()}
                title="Attach files"
              >
                <Plus size={16} className="text-zinc-300" />
              </div>
            </div>
          </div>
          <Button 
            className="w-24 h-11 shadow-lg bg-primary/80 hover:bg-primary/90 text-white flex items-center gap-2"
            variant="default"
            onClick={handleSendMessage}
            disabled={!prompt.trim()}
          >
            <span>Send</span>
            <Send size={16} />
          </Button>
        </div>
      </div>
    </div>
  )
}

export default CodexPage
