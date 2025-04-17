import React, { useState } from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card'
import { Button } from '../components/ui/button'
import { Input } from '../components/ui/input'
import { Label } from '../components/ui/label'
import { Progress } from '../components/ui/progress'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../components/ui/select'
import { Switch } from '../components/ui/switch'
import { Mic, Upload, RefreshCw, User, Database, FileAudio, Activity } from 'lucide-react'

interface AudioFile {
  id: string;
  name: string;
  size: number;
  duration: number;
}

interface Entity {
  id: string;
  name: string;
}

interface KnowledgeBase {
  id: string;
  name: string;
  selected: boolean;
}

const VocalithPage: React.FC = () => {
  // Processing state
  const [isProcessing, setIsProcessing] = useState<boolean>(false);
  const [progress, setProgress] = useState<number>(0);
  const [tokensTranscribed, setTokensTranscribed] = useState<number>(0);
  
  // Audio files state
  const [audioFiles, setAudioFiles] = useState<AudioFile[]>([
    { id: '1', name: 'interview_session.mp3', size: 12500000, duration: 1450 },
    { id: '2', name: 'meeting_recording.wav', size: 28750000, duration: 3250 },
  ]);
  
  // Transcription output
  const [transcriptionText, setTranscriptionText] = useState<string>(
    `[00:00:15] Speaker 1: Welcome everyone to our quarterly planning meeting. Today we'll be discussing the roadmap for Q3 and Q4 2025.

[00:00:23] Speaker 2: Before we dive in, can we get a quick overview of how Q2 went?

[00:00:28] Speaker 1: Absolutely. In Q2, we exceeded our revenue targets by 15% and successfully launched three new features that were very well received by customers.

[00:00:40] Speaker 3: The customer feedback on the new AI assistant has been particularly positive. We're seeing a 30% increase in daily active users for accounts that have enabled it.

[00:00:52] Speaker 2: That's impressive. Are we planning to expand on those AI capabilities in the coming quarters?

[00:00:58] Speaker 1: Yes, that's actually one of our key priorities for Q3. We're looking to integrate more advanced NLP capabilities and potentially add voice interaction.`
  );
  
  // Display settings
  const [viewMode, setViewMode] = useState<'raw' | 'dialog'>('dialog');
  
  // Entity selection
  const [entities, setEntities] = useState<Entity[]>([
    { id: '1', name: 'John Smith (CEO)' },
    { id: '2', name: 'Sarah Johnson (CTO)' },
    { id: '3', name: 'Michael Williams (Product Manager)' },
    { id: '4', name: 'Emily Davis (Head of Marketing)' },
  ]);
  const [selectedEntityId, setSelectedEntityId] = useState<string>('');
  
  // Knowledge bases
  const [knowledgeBases, setKnowledgeBases] = useState<KnowledgeBase[]>([
    { id: '1', name: 'Company Documentation', selected: true },
    { id: '2', name: 'Product Specifications', selected: false },
    { id: '3', name: 'Meeting Minutes', selected: true },
    { id: '4', name: 'Customer Feedback', selected: false },
  ]);
  
  // File input reference
  const fileInputRef = React.useRef<HTMLInputElement>(null);
  
  // Toggle knowledge base selection
  const toggleKnowledgeBase = (id: string) => {
    setKnowledgeBases(prev => 
      prev.map(kb => kb.id === id ? { ...kb, selected: !kb.selected } : kb)
    );
  };
  
  // Format duration
  const formatDuration = (seconds: number): string => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const remainingSeconds = seconds % 60;
    
    return `${hours > 0 ? hours + 'h ' : ''}${minutes}m ${remainingSeconds}s`;
  };
  
  // Format file size
  const formatFileSize = (bytes: number): string => {
    if (bytes < 1024) return bytes + ' B';
    else if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB';
    else return (bytes / 1048576).toFixed(1) + ' MB';
  };
  
  // Handle file upload
  const handleFileUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      const newFiles: AudioFile[] = Array.from(e.target.files).map(file => ({
        id: Math.random().toString(36).substring(2, 11),
        name: file.name,
        size: file.size,
        duration: Math.floor(Math.random() * 3600) // For demo purposes only
      }));
      
      setAudioFiles(prev => [...prev, ...newFiles]);
      e.target.value = ''; // Reset to allow selecting the same file again
    }
  };
  
  // Toggle processing state with simulated progress
  const toggleProcessing = () => {
    if (isProcessing) {
      setIsProcessing(false);
      setProgress(0);
    } else {
      setIsProcessing(true);
      setProgress(0);
      setTokensTranscribed(0);
      
      // Simulate transcription progress
      const interval = setInterval(() => {
        setProgress(prev => {
          const newProgress = prev + (Math.random() * 2);
          setTokensTranscribed(Math.floor(newProgress * 20));
          
          if (newProgress >= 100) {
            clearInterval(interval);
            setTimeout(() => setIsProcessing(false), 500);
            return 100;
          }
          return newProgress;
        });
      }, 200);
    }
  };

  return (
    <div className="grid grid-cols-12 gap-3 h-[calc(100vh-4rem-3rem)]">
      {/* Top row */}
      <div className="col-span-12 grid grid-cols-12 gap-3">
        {/* Activity indicator */}
        <div className="col-span-2 flex items-center justify-center">
          <div className="w-24 h-24 rounded-full flex items-center justify-center relative">
            <div className={`absolute inset-0 rounded-full ${isProcessing 
                ? 'border-4 border-primary border-t-transparent animate-spin' 
                : 'border-4 border-zinc-700'
              }`}></div>
            <div className="text-center">
              <Mic 
                size={28} 
                className={isProcessing ? "text-primary animate-pulse" : "text-zinc-400"} 
              />
            </div>
          </div>
        </div>
        
        {/* Transcription progress */}
        <div className="col-span-6">
          <Card className="h-full border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
            <CardContent className="p-4 flex flex-col justify-center h-full">
              <div className="space-y-4">
                <div className="flex items-center justify-between mb-1">
                  <span className="text-sm font-medium text-zinc-300">Transcription Progress</span>
                  <span className="text-sm font-medium text-zinc-400">{progress.toFixed(0)}%</span>
                </div>
                <Progress value={progress} className="h-2 bg-zinc-700" />
                
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <div className="text-xs text-zinc-500 mb-1">Status</div>
                    <div className="text-sm font-medium text-zinc-300 flex items-center">
                      {isProcessing ? (
                        <>
                          <RefreshCw size={14} className="mr-1 animate-spin text-primary" />
                          <span>Processing</span>
                        </>
                      ) : (
                        <>
                          <div className="w-2 h-2 rounded-full bg-green-500 mr-2"></div>
                          <span>Ready</span>
                        </>
                      )}
                    </div>
                  </div>
                  
                  <div>
                    <div className="text-xs text-zinc-500 mb-1">Tokens Transcribed</div>
                    <div className="text-sm font-medium text-zinc-300">{tokensTranscribed}</div>
                  </div>
                  
                  <div>
                    <div className="text-xs text-zinc-500 mb-1">Display Mode</div>
                    <div className="flex items-center space-x-2">
                      <Switch 
                        id="view-mode" 
                        checked={viewMode === 'dialog'} 
                        onCheckedChange={(checked) => setViewMode(checked ? 'dialog' : 'raw')}
                      />
                      <Label htmlFor="view-mode" className="text-sm text-zinc-300">
                        {viewMode === 'dialog' ? 'Dialog View' : 'Raw Text'}
                      </Label>
                    </div>
                  </div>
                  
                  <div>
                    <div className="text-xs text-zinc-500 mb-1">Total Duration</div>
                    <div className="text-sm font-medium text-zinc-300">
                      {formatDuration(audioFiles.reduce((total, file) => total + file.duration, 0))}
                    </div>
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>
        </div>
        
        {/* File upload */}
        <div className="col-span-4">
          <Card className="h-full border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
            <CardHeader className="p-3 relative z-10">
              <div className="flex items-center gap-2">
                <Upload size={16} className="text-primary/80" />
                <CardTitle className="text-primary-foreground text-sm">Audio File Upload</CardTitle>
              </div>
            </CardHeader>
            <CardContent className="p-3 pt-0 relative z-10">
              <div
                className="border-2 border-dashed border-zinc-700 rounded-lg p-3 mb-2 cursor-pointer hover:border-primary/50 transition-colors"
                onClick={() => fileInputRef.current?.click()}
              >
                <div className="flex flex-col items-center justify-center text-center">
                  <FileAudio className="h-8 w-8 text-zinc-500 mb-1" />
                  <p className="text-xs text-zinc-400">Drag audio files here</p>
                  <p className="text-xs text-zinc-500">MP3, WAV, M4A, FLAC</p>
                </div>
                <input
                  type="file"
                  multiple
                  accept=".mp3,.wav,.m4a,.flac"
                  className="hidden"
                  ref={fileInputRef}
                  onChange={handleFileUpload}
                />
              </div>
              
              <div className="max-h-[100px] overflow-y-auto scrollbar-thin scrollbar-thumb-zinc-700">
                {audioFiles.map(file => (
                  <div 
                    key={file.id}
                    className="flex items-center justify-between py-1 px-2 text-xs border-b border-zinc-700/30 last:border-0"
                  >
                    <div className="flex items-center">
                      <Activity size={12} className="text-primary/60 mr-1.5" />
                      <span className="text-zinc-300 truncate max-w-[160px]">{file.name}</span>
                    </div>
                    <div className="text-zinc-500 flex gap-2">
                      <span>{formatDuration(file.duration)}</span>
                      <span>|</span>
                      <span>{formatFileSize(file.size)}</span>
                    </div>
                  </div>
                ))}
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
      
      {/* Main content row */}
      <div className="col-span-8 row-span-2">
        <Card className="h-full border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Mic size={18} className="text-primary/80" />
                <CardTitle className="text-primary-foreground">Transcription Output</CardTitle>
              </div>
              <div className="flex items-center gap-2">
                <Button variant="ghost" size="sm" className="h-8 gap-1 text-xs">
                  <RefreshCw size={14} />
                  <span>Refresh</span>
                </Button>
                <Button 
                  size="sm" 
                  className={`h-8 gap-1 text-xs ${isProcessing ? 'bg-red-700/80 hover:bg-red-700/90' : 'bg-primary/80 hover:bg-primary/90'}`}
                  onClick={toggleProcessing}
                >
                  {isProcessing ? (
                    <>
                      <span>Stop</span>
                    </>
                  ) : (
                    <>
                      <span>Start Transcription</span>
                    </>
                  )}
                </Button>
              </div>
            </div>
          </CardHeader>
          <CardContent className="p-0 relative z-10 h-[calc(100%-4rem)] overflow-auto">
            {viewMode === 'dialog' ? (
              <div className="p-4 bg-zinc-800/20 h-full text-zinc-300 whitespace-pre-wrap text-sm">
                {transcriptionText.split('\n\n').map((block, idx) => {
                  const match = block.match(/\[(.*?)\] (Speaker \d+): (.*)/);
                  if (!match) return <p key={idx} className="mb-2">{block}</p>;
                  
                  const [_, timestamp, speaker, text] = match;
                  
                  return (
                    <div key={idx} className="mb-4">
                      <div className="flex items-center mb-1">
                        <div className="text-xs text-zinc-500 font-mono">{timestamp}</div>
                        <div className="ml-2 px-2 py-0.5 rounded-full bg-zinc-700/60 text-xs font-medium text-primary-foreground">
                          {speaker}
                        </div>
                      </div>
                      <div className="pl-16 text-zinc-200">{text}</div>
                    </div>
                  );
                })}
              </div>
            ) : (
              <div className="p-4 bg-zinc-800/20 h-full font-mono text-zinc-300 whitespace-pre-wrap text-sm">
                {transcriptionText}
              </div>
            )}
          </CardContent>
        </Card>
      </div>
      
      {/* Right column */}
      <div className="col-span-4 flex flex-col gap-3">
        {/* Done button */}
        <Button className="h-12 bg-green-700/80 hover:bg-green-700/90 text-white">
          Done
        </Button>
        
        {/* Who is talking */}
        <Card className="border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center gap-2">
              <User size={18} className="text-primary/80" />
              <CardTitle className="text-primary-foreground">Who is talking</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4 pt-2 relative z-10">
            <div className="space-y-4">
              <Select value={selectedEntityId} onValueChange={setSelectedEntityId}>
                <SelectTrigger>
                  <SelectValue placeholder="Select a speaker..." />
                </SelectTrigger>
                <SelectContent>
                  {entities.map(entity => (
                    <SelectItem key={entity.id} value={entity.id}>
                      {entity.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              
              <div className="flex gap-2">
                <Input 
                  placeholder="Add new speaker..."
                  className="bg-zinc-800/60 border-zinc-700 text-zinc-200 flex-1"
                />
                <Button variant="outline" className="border-zinc-700 text-zinc-300">
                  Add
                </Button>
              </div>
              
              <div className="space-y-2">
                <div className="text-xs text-zinc-500 mb-1">Current speakers:</div>
                <div className="space-y-2">
                  {entities.map(entity => (
                    <div key={entity.id} className="flex items-center justify-between group">
                      <div className="flex items-center">
                        <div className="w-2 h-2 rounded-full bg-primary/80 mr-2"></div>
                        <span className="text-sm text-zinc-300">{entity.name}</span>
                      </div>
                      <Button 
                        variant="ghost" 
                        size="sm" 
                        className="h-6 w-6 p-0 opacity-0 group-hover:opacity-100 text-zinc-400 hover:text-zinc-200"
                      >
                        <span className="sr-only">Remove</span>
                        ×
                      </Button>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
        
        {/* Knowledge base */}
        <Card className="flex-grow border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center gap-2">
              <Database size={18} className="text-primary/80" />
              <CardTitle className="text-primary-foreground">Knowledge Base</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4 pt-2 relative z-10">
            <div className="grid grid-cols-1 gap-2">
              {knowledgeBases.map(kb => (
                <div key={kb.id} className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Switch
                      id={`kb-${kb.id}`}
                      checked={kb.selected}
                      onCheckedChange={() => toggleKnowledgeBase(kb.id)}
                    />
                    <Label
                      htmlFor={`kb-${kb.id}`}
                      className="text-sm font-medium text-zinc-300"
                    >
                      {kb.name}
                    </Label>
                  </div>
                  <div className="text-xs text-zinc-500">
                    {kb.selected ? 'Enabled' : 'Disabled'}
                  </div>
                </div>
              ))}
            </div>
            
            <div className="mt-4">
              <div className="text-xs text-zinc-500 mb-1">Push to Knowledge Base:</div>
              <div className="flex gap-2">
                <Input 
                  placeholder="Name for new entry..."
                  className="bg-zinc-800/60 border-zinc-700 text-zinc-200 flex-1"
                />
                <Button className="bg-primary/80 hover:bg-primary/90">
                  Push
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

export default VocalithPage
