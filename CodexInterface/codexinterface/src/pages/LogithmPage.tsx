import React, { useState } from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card'
import { Button } from '../components/ui/button'
import { Input } from '../components/ui/input'
import { Textarea } from '../components/ui/textarea'
import { Checkbox } from '../components/ui/checkbox'
import { Label } from '../components/ui/label'
import { Slider } from '../components/ui/slider'
import { Upload, FileText, Settings, Database, BookOpen, Zap } from 'lucide-react'

// Type definitions
interface DocumentFile {
  id: string;
  name: string;
  size: number;
  type: string;
}

interface KnowledgeBase {
  id: string;
  name: string;
  selected: boolean;
}

interface EmbeddingModel {
  id: string;
  name: string;
}

const LogithmPage: React.FC = () => {
  // Uploaded documents state
  const [documents, setDocuments] = useState<DocumentFile[]>([
    { id: '1', name: 'quarterly_report_q1_2025.pdf', size: 1250000, type: 'application/pdf' },
    { id: '2', name: 'product_specification.docx', size: 520000, type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' },
    { id: '3', name: 'research_paper.pdf', size: 2400000, type: 'application/pdf' },
  ]);
  
  // Selected document state
  const [selectedDocumentId, setSelectedDocumentId] = useState<string>('1');
  
  // Document content placeholder
  const [documentContent, setDocumentContent] = useState<string>(
    "# Quarterly Report Q1 2025\n\n## Executive Summary\nIn the first quarter of 2025, the company has shown significant growth across all key metrics. Revenue increased by 15% compared to the previous quarter, while operational costs were reduced by 7% through improved efficiency measures.\n\n## Financial Highlights\n- Total Revenue: $12.5M (↑15% QoQ)\n- Gross Margin: 68% (↑3% QoQ)\n- Operating Expenses: $4.2M (↓7% QoQ)\n- Net Profit: $4.3M (↑22% QoQ)\n\n## Product Development\nThe R&D team has successfully completed the development of three new features:\n1. AI-assisted document processing\n2. Advanced data visualization tools\n3. Integrated knowledge management system\n\nThese features are scheduled for release in Q2 2025, with the marketing campaign already in preparation.\n\n## Market Analysis\nThe competitive landscape continues to evolve with new entrants in the AI space. Our product maintains a technological advantage in document processing efficiency and accuracy. Market share increased to 24% (↑2% from Q4 2024)."
  );
  
  // LLM transformation prompt
  const [transformationPrompt, setTransformationPrompt] = useState<string>(
    "Extract and summarize the key financial data and business insights from this document."
  );
  
  // Chunking settings
  const [chunkSize, setChunkSize] = useState<number>(512);
  const [chunkOverlap, setChunkOverlap] = useState<number>(50);
  // Chunking strategy: 'recursive' or 'html'
  const [chunkStrategy, setChunkStrategy] = useState<'recursive' | 'html'>('recursive');
  
  // Embedding models
  const [embeddingModels, setEmbeddingModels] = useState<EmbeddingModel[]>([
    { id: 'ada-002', name: 'text-embedding-ada-002' },
    { id: 'e5-large', name: 'E5-large' },
    { id: 'mpnet-base', name: 'MPNet-base-v2' },
    { id: 'glove', name: 'GloVe (6B)' }
  ]);
  const [selectedModelId, setSelectedModelId] = useState<string>("ada-002");
  
  // Processing options
  const [removeDuplicates, setRemoveDuplicates] = useState<boolean>(false);
  const [stripMetadata, setStripMetadata] = useState<boolean>(true);
  
  // Knowledge bases
  const [knowledgeBases, setKnowledgeBases] = useState<KnowledgeBase[]>([
    { id: '1', name: 'Financial Reports', selected: true },
    { id: '2', name: 'Product Documentation', selected: false },
    { id: '3', name: 'Research Materials', selected: true },
    { id: '4', name: 'Meeting Notes', selected: false },
    { id: '5', name: 'Customer Feedback', selected: false },
  ]);
  const [newKnowledgeBase, setNewKnowledgeBase] = useState<string>("");
  
  // File input reference
  const fileInputRef = React.useRef<HTMLInputElement>(null);
  
  // Format file size
  const formatFileSize = (bytes: number): string => {
    if (bytes < 1024) return bytes + ' B';
    else if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB';
    else return (bytes / 1048576).toFixed(1) + ' MB';
  };
  
  // Handle file upload
  const handleFileUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      const newFiles: DocumentFile[] = Array.from(e.target.files).map(file => ({
        id: Math.random().toString(36).substring(2, 11),
        name: file.name,
        size: file.size,
        type: file.type
      }));
      
      setDocuments(prev => [...prev, ...newFiles]);
      if (newFiles.length > 0 && !selectedDocumentId) {
        setSelectedDocumentId(newFiles[0].id);
      }
      e.target.value = ''; // Reset to allow selecting the same file again
    }
  };
  
  // Handle knowledge base toggle
  const toggleKnowledgeBase = (id: string) => {
    setKnowledgeBases(prev => 
      prev.map(kb => kb.id === id ? { ...kb, selected: !kb.selected } : kb)
    );
  };
  
  // Add new knowledge base
  const addKnowledgeBase = () => {
    if (newKnowledgeBase.trim()) {
      const newId = (knowledgeBases.length + 1).toString();
      setKnowledgeBases(prev => [
        ...prev,
        {
          id: newId,
          name: newKnowledgeBase.trim(),
          selected: true
        }
      ]);
      setNewKnowledgeBase("");
    }
  };
  
  // Handle keypress for new knowledge base input
  const handleNewKnowledgeBaseKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      addKnowledgeBase();
    }
  };
  
  // Get selected document
  const selectedDocument = documents.find(d => d.id === selectedDocumentId);
  // Compute text chunks based on strategy
  const chunks = React.useMemo(() => {
    const text = documentContent || '';
    if (chunkStrategy === 'html') {
      // HTML block-level split
      try {
        const parser = new DOMParser();
        const doc = parser.parseFromString(text, 'text/html');
        const blocks = Array.from(doc.body.querySelectorAll('p,div,li,h1,h2,h3,h4,h5,h6'));
        if (blocks.length > 0) {
          return blocks.map(el => el.textContent || '');
        }
      } catch {
        // fallback to full text
      }
    }
    // Default: recursive character splitting
    const size = chunkSize;
    const overlap = chunkOverlap;
    const out: string[] = [];
    let start = 0;
    while (start < text.length) {
      const end = Math.min(text.length, start + size);
      out.push(text.slice(start, end));
      start = end - overlap;
      if (start < 0) start = 0;
      if (end >= text.length) break;
    }
    return out;
  }, [documentContent, chunkStrategy, chunkSize, chunkOverlap]);

  // Colors for chunk highlighting
  const chunkColors = ['rgba(255,0,0,0.1)', 'rgba(0,255,0,0.1)', 'rgba(0,0,255,0.1)', 'rgba(255,255,0,0.1)', 'rgba(0,255,255,0.1)', 'rgba(255,0,255,0.1)'];

  return (
    <div className="grid grid-cols-12 gap-3 h-[calc(100vh-4rem-3rem)]">
      {/* Left Column - Chunking settings, Document selector, Document content */}
      <div className="col-span-7 flex flex-col gap-3">
        {/* Chunking and embedding settings */}
        <Card className="border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center gap-2">
              <Settings size={18} className="text-primary/80" />
              <CardTitle className="text-primary-foreground">Chunking and Embedding Settings</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4 pt-2 relative z-10">
            <div className="grid grid-cols-2 gap-6">
              <div className="space-y-4">
                <div className="space-y-2">
                  <div className="flex justify-between">
                    <Label htmlFor="chunk-size">Chunk Size: {chunkSize} tokens</Label>
                  </div>
                  <Slider 
                    id="chunk-size"
                    value={[chunkSize]} 
                    min={128} 
                    max={2048} 
                    step={128} 
                    onValueChange={(value: number[]) => setChunkSize(value[0])}
                    className="w-full"
                  />
                </div>
                
                <div className="space-y-2">
                  <div className="flex justify-between">
                    <Label htmlFor="chunk-overlap">Chunk Overlap: {chunkOverlap} tokens</Label>
                  </div>
                  <Slider 
                    id="chunk-overlap"
                    value={[chunkOverlap]} 
                    min={0} 
                    max={200} 
                    step={10} 
                    onValueChange={(value: number[]) => setChunkOverlap(value[0])}
                    className="w-full"
                  />
                </div>
                {/* Chunking strategy selector */}
                <div className="space-y-2">
                  <Label htmlFor="chunk-strategy">Chunking Strategy</Label>
                  <select
                    id="chunk-strategy"
                    value={chunkStrategy}
                    onChange={(e) => setChunkStrategy(e.target.value as 'recursive' | 'html')}
                    className="w-full h-10 rounded-md border border-zinc-700 bg-zinc-800/60 px-3 py-2 text-sm text-zinc-200"
                  >
                    <option value="recursive">Recursive Character Split</option>
                    <option value="html">HTML Block Split</option>
                  </select>
                </div>
              </div>
              
              <div className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="embedding-model">Embedding Model</Label>
                  <select
                    id="embedding-model"
                    value={selectedModelId}
                    onChange={(e: React.ChangeEvent<HTMLSelectElement>) => setSelectedModelId(e.target.value)}
                    className="w-full h-10 rounded-md border border-zinc-700 bg-zinc-800/60 px-3 py-2 text-sm text-zinc-200"
                  >
                    {embeddingModels.map(model => (
                      <option key={model.id} value={model.id}>{model.name}</option>
                    ))}
                  </select>
                </div>
                
                <div className="flex items-center space-x-2 mt-2">
                  <Checkbox
                    id="remove-duplicates"
                    checked={removeDuplicates}
                    onCheckedChange={() => setRemoveDuplicates(!removeDuplicates)}
                  />
                  <Label htmlFor="remove-duplicates">Remove duplicate chunks</Label>
                </div>
                
                <div className="flex items-center space-x-2">
                  <Checkbox
                    id="strip-metadata"
                    checked={stripMetadata}
                    onCheckedChange={() => setStripMetadata(!stripMetadata)}
                  />
                  <Label htmlFor="strip-metadata">Strip metadata from documents</Label>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
        
        {/* Document Selector */}
        <div className="h-12 border border-zinc-800 rounded-md bg-zinc-800/40 shadow-[0_0_15px_rgba(0,0,0,0.5)] flex items-center px-4">
          <div className="flex-1 flex items-center gap-4 overflow-x-auto scrollbar-thin scrollbar-thumb-zinc-700 scrollbar-track-zinc-800/20">
            {documents.map(doc => (
              <div 
                key={doc.id}
                className={`flex items-center gap-2 px-3 py-1.5 rounded-md cursor-pointer whitespace-nowrap ${
                  selectedDocumentId === doc.id 
                    ? 'bg-primary/20 text-primary border border-primary/30' 
                    : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/70 border border-transparent'
                }`}
                onClick={() => setSelectedDocumentId(doc.id)}
              >
                <FileText size={14} />
                <span className="text-sm">{doc.name}</span>
              </div>
            ))}
          </div>
        </div>
        
        {/* Document Content */}
        <Card className="flex-grow border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <BookOpen size={18} className="text-primary/80" />
                <CardTitle className="text-primary-foreground">Document</CardTitle>
              </div>
              {selectedDocument && (
                <div className="text-xs text-zinc-400">
                  {formatFileSize(selectedDocument.size)} | {selectedDocument.name.split('.').pop()?.toUpperCase()}
                </div>
              )}
            </div>
          </CardHeader>
          <CardContent className="p-0 relative z-10 h-[calc(100%-4rem)] overflow-auto">
            <div className="p-4 bg-zinc-800/20 h-full font-mono text-zinc-300 whitespace-pre-wrap text-sm">
              {documentContent}
            </div>
          </CardContent>
        </Card>
      </div>
      
      {/* Right Column - File Upload, LLM Transformation, Knowledge Bases */}
      <div className="col-span-5 flex flex-col gap-3">
        {/* File Upload Window */}
        <Card className="border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center gap-2">
              <Upload size={18} className="text-primary/80" />
              <CardTitle className="text-primary-foreground">File Upload</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4 pt-2 relative z-10">
            <div
              className="border-2 border-dashed border-zinc-700 rounded-lg p-4 mb-4 cursor-pointer hover:border-primary/50 transition-colors"
              onClick={() => fileInputRef.current?.click()}
            >
              <div className="flex flex-col items-center justify-center text-center">
                <Upload className="h-10 w-10 text-zinc-500 mb-2" />
                <p className="text-sm text-zinc-400 mb-1">Drag and drop files here</p>
                <p className="text-xs text-zinc-500">PDF, DOCX, TXT, MD, HTML</p>
              </div>
              <input
                type="file"
                multiple
                accept=".pdf,.docx,.txt,.md,.html"
                className="hidden"
                ref={fileInputRef}
                onChange={handleFileUpload}
              />
            </div>
            
            <div className="text-xs text-zinc-500 flex justify-between items-center">
              <span>Maximum file size: 20MB</span>
              <span>{documents.length} documents</span>
            </div>
          </CardContent>
        </Card>
        
        {/* LLM Transformation Prompt and Button */}
        <div className="flex gap-3">
          <div className="flex-grow">
            <Card className="h-full border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
              <CardHeader className="p-3 pb-0 relative z-10">
                <CardTitle className="text-sm text-primary-foreground">LLM Transformation Prompt</CardTitle>
              </CardHeader>
              <CardContent className="p-3 pt-2 relative z-10">
                <Textarea 
                  value={transformationPrompt}
                  onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) => setTransformationPrompt(e.target.value)}
                  placeholder="Enter instructions for transforming the document..."
                  className="min-h-[100px] bg-zinc-800/60 border-zinc-700 text-zinc-200 resize-none"
                />
              </CardContent>
            </Card>
          </div>
          
          <Button className="h-full min-w-[120px] bg-primary/80 hover:bg-primary/90">
            <Zap size={16} className="mr-2" />
            Transform
          </Button>
        </div>
        
        {/* Knowledge Bases */}
        <Card className="flex-grow border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
          <CardHeader className="p-4 pb-2 relative z-10">
            <div className="flex items-center gap-2">
              <Database size={18} className="text-primary/80" />
              <CardTitle className="text-primary-foreground">Knowledge Bases</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4 pt-2 relative z-10">
            <div className="grid grid-cols-2 gap-3">
              {knowledgeBases.map(kb => (
                <div key={kb.id} className="flex items-center space-x-2">
                  <Checkbox 
                    id={`kb-${kb.id}`} 
                    checked={kb.selected}
                    onCheckedChange={() => toggleKnowledgeBase(kb.id)}
                  />
                  <Label
                    htmlFor={`kb-${kb.id}`}
                    className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
                  >
                    {kb.name}
                  </Label>
                </div>
              ))}
            </div>
            
            <div className="mt-4">
              <Input 
                placeholder="Create new knowledge base..."
                value={newKnowledgeBase}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => setNewKnowledgeBase(e.target.value)}
                onKeyDown={handleNewKnowledgeBaseKeyDown}
                className="bg-zinc-800/60 border-zinc-700 text-zinc-200"
              />
            </div>
          </CardContent>
        </Card>
        
        {/* Action Buttons */}
        <div className="grid grid-cols-1 gap-3">
          <Button className="bg-primary/80 hover:bg-primary/90 h-12">
            Add this document
          </Button>
          
          <Button className="bg-zinc-700/80 hover:bg-zinc-700 text-zinc-200 h-12">
            Add all documents
          </Button>
        </div>
      </div>
      {/* Right Column - Document View */}
      <div className="col-span-5 flex flex-col gap-3 h-full">
        <Card className="flex flex-col flex-grow border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden">
          <CardHeader className="p-4 flex-none z-10">
            <div className="flex items-center gap-2">
              <FileText size={18} className="text-primary/80" />
              <CardTitle className="text-primary-foreground">Document View</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4 flex-grow overflow-auto">
            <div style={{ whiteSpace: 'pre-wrap' }}>
              {chunks.map((chunk, idx) => (
                <span
                  key={idx}
                  style={{ backgroundColor: chunkColors[idx % chunkColors.length] }}
                >
                  {chunk}
                </span>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

export default LogithmPage
