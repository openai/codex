/**
 * Tool Builder UI - Simple Dashboard
 * Interactive web interface for the Codex Tool Builder System
 */

import React, { useState, useEffect } from 'react';
import Head from 'next/head';
import { 
  Wand2, 
  Github, 
  Cloud, 
  Terminal, 
  Rocket, 
  CheckCircle, 
  XCircle, 
  Clock,
  Plus,
  Eye,
  Trash2
} from 'lucide-react';
import { generateTool, getTools, deleteTool } from '../lib/api';

interface Tool {
  id: string;
  name: string;
  description: string;
  status: string;
  created_at: string;
  github_url?: string;
  codespace_url?: string;
}

export default function Home() {
  const [tools, setTools] = useState<Tool[]>([]);
  const [isCreating, setIsCreating] = useState(false);
  const [newTool, setNewTool] = useState({ name: '', description: '' });
  const [activeTab, setActiveTab] = useState('create');

  useEffect(() => {
    loadTools();
  }, []);

  const loadTools = async () => {
    try {
      const toolsList = await getTools();
      setTools(toolsList);
    } catch (error) {
      console.error('Failed to load tools:', error);
    }
  };

  const handleCreateTool = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!newTool.name || !newTool.description) {
      alert('Please provide both tool name and description');
      return;
    }

    setIsCreating(true);
    
    try {
      const tool = await generateTool(newTool);
      setTools(prev => [tool, ...prev]);
      setNewTool({ name: '', description: '' });
      setActiveTab('tools');
      alert(`Tool creation started! ${tool.name} is being generated automatically`);
    } catch (error) {
      alert('Failed to create tool');
    } finally {
      setIsCreating(false);
    }
  };

  const handleDeleteTool = async (toolId: string) => {
    try {
      await deleteTool(toolId);
      setTools(prev => prev.filter(tool => tool.id !== toolId));
      alert('Tool deleted successfully');
    } catch (error) {
      alert('Failed to delete tool');
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'completed':
        return <CheckCircle className="h-4 w-4 text-green-500" />;
      case 'failed':
      case 'error':
        return <XCircle className="h-4 w-4 text-red-500" />;
      case 'running':
      case 'initializing':
        return <Clock className="h-4 w-4 text-blue-500 animate-spin" />;
      default:
        return <Clock className="h-4 w-4 text-gray-500" />;
    }
  };

  return (
    <>
      <Head>
        <title>Codex Tool Builder - Autonomous CLI Tool Generation</title>
        <meta name="description" content="AI-powered tool generation and deployment" />
        <link rel="icon" href="/favicon.ico" />
      </Head>

      <div className="min-h-screen bg-gradient-to-br from-blue-50 to-indigo-100 p-8">
        <div className="container mx-auto max-w-4xl">
          {/* Header */}
          <div className="text-center mb-8">
            <h1 className="text-4xl font-bold text-gray-900 mb-2">
              🤖 Codex Tool Builder
            </h1>
            <p className="text-xl text-gray-600">
              Generate complete CLI tools with AI in minutes, not hours
            </p>
          </div>

          {/* Tab Navigation */}
          <div className="flex justify-center mb-6">
            <div className="bg-white rounded-lg p-1 shadow-md">
              <button
                onClick={() => setActiveTab('create')}
                className={`px-4 py-2 rounded-md font-medium ${
                  activeTab === 'create' 
                    ? 'bg-blue-500 text-white' 
                    : 'text-gray-700 hover:bg-gray-100'
                }`}
              >
                <Plus className="h-4 w-4 inline mr-2" />
                Create Tool
              </button>
              <button
                onClick={() => setActiveTab('tools')}
                className={`px-4 py-2 rounded-md font-medium ${
                  activeTab === 'tools' 
                    ? 'bg-blue-500 text-white' 
                    : 'text-gray-700 hover:bg-gray-100'
                }`}
              >
                <Eye className="h-4 w-4 inline mr-2" />
                My Tools ({tools.length})
              </button>
            </div>
          </div>

          {/* Create Tool Tab */}
          {activeTab === 'create' && (
            <div className="bg-white rounded-lg shadow-lg p-8 max-w-2xl mx-auto">
              <div className="flex items-center gap-2 mb-4">
                <Wand2 className="h-5 w-5" />
                <h2 className="text-2xl font-bold">Generate New CLI Tool</h2>
              </div>
              <p className="text-gray-600 mb-6">
                Describe what you want your tool to do, and AI will generate the complete application with GitHub repo, CI/CD, and cloud development environment.
              </p>
              
              <form onSubmit={handleCreateTool} className="space-y-4">
                <div>
                  <label className="block text-sm font-medium mb-2">
                    Tool Name
                  </label>
                  <input
                    type="text"
                    placeholder="my-awesome-tool"
                    value={newTool.name}
                    onChange={(e) => setNewTool(prev => ({ ...prev, name: e.target.value }))}
                    className="w-full p-3 border border-gray-300 rounded-md focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                  />
                </div>
                
                <div>
                  <label className="block text-sm font-medium mb-2">
                    Description
                  </label>
                  <textarea
                    placeholder="Describe what your tool should do..."
                    value={newTool.description}
                    onChange={(e) => setNewTool(prev => ({ ...prev, description: e.target.value }))}
                    className="w-full p-3 border border-gray-300 rounded-md focus:ring-2 focus:ring-blue-500 focus:border-transparent min-h-[100px]"
                  />
                </div>

                <button 
                  type="submit" 
                  disabled={isCreating}
                  className="w-full bg-blue-500 text-white py-3 px-6 rounded-md hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                >
                  {isCreating ? (
                    <>
                      <Clock className="h-4 w-4 animate-spin" />
                      Generating Tool...
                    </>
                  ) : (
                    <>
                      <Rocket className="h-4 w-4" />
                      Generate Tool
                    </>
                  )}
                </button>
              </form>

              {/* Quick Examples */}
              <div className="mt-6 p-4 bg-gray-50 rounded-lg">
                <h4 className="font-medium mb-2">💡 Quick Examples:</h4>
                <div className="space-y-2 text-sm">
                  <button 
                    type="button"
                    onClick={() => setNewTool({
                      name: 'todo-manager',
                      description: 'manages todo lists with priorities and due dates'
                    })}
                    className="block text-left text-blue-600 hover:text-blue-800"
                  >
                    • "manages todo lists with priorities and due dates"
                  </button>
                  <button 
                    type="button"
                    onClick={() => setNewTool({
                      name: 'log-analyzer',
                      description: 'analyzes server logs and extracts error patterns'
                    })}
                    className="block text-left text-blue-600 hover:text-blue-800"
                  >
                    • "analyzes server logs and extracts error patterns"
                  </button>
                  <button 
                    type="button"
                    onClick={() => setNewTool({
                      name: 'deploy-helper',
                      description: 'automates application deployment across environments'
                    })}
                    className="block text-left text-blue-600 hover:text-blue-800"
                  >
                    • "automates application deployment across environments"
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* Tools List Tab */}
          {activeTab === 'tools' && (
            <div className="space-y-4">
              {tools.length === 0 ? (
                <div className="bg-white rounded-lg shadow-lg p-8 text-center">
                  <Terminal className="h-12 w-12 mx-auto text-gray-400 mb-4" />
                  <h3 className="text-lg font-medium text-gray-900 mb-2">
                    No tools generated yet
                  </h3>
                  <p className="text-gray-600 mb-4">
                    Create your first AI-generated CLI tool to get started!
                  </p>
                  <button 
                    onClick={() => setActiveTab('create')}
                    className="bg-blue-500 text-white px-4 py-2 rounded-md hover:bg-blue-600 flex items-center gap-2 mx-auto"
                  >
                    <Plus className="h-4 w-4" />
                    Create First Tool
                  </button>
                </div>
              ) : (
                tools.map((tool) => (
                  <div key={tool.id} className="bg-white rounded-lg shadow-lg p-6">
                    <div className="flex items-start justify-between">
                      <div className="flex-1">
                        <div className="flex items-center gap-2 mb-2">
                          <h3 className="text-lg font-semibold">{tool.name}</h3>
                          <span className={`px-2 py-1 rounded-full text-xs font-medium ${
                            tool.status === 'completed' ? 'bg-green-100 text-green-800' :
                            tool.status === 'failed' ? 'bg-red-100 text-red-800' :
                            'bg-blue-100 text-blue-800'
                          }`}>
                            {getStatusIcon(tool.status)}
                            <span className="ml-1">{tool.status}</span>
                          </span>
                        </div>
                        
                        <p className="text-gray-600 mb-3">{tool.description}</p>
                        
                        <div className="flex items-center gap-4 text-sm text-gray-500">
                          <span>Created: {new Date(tool.created_at).toLocaleDateString()}</span>
                          {tool.github_url && (
                            <a 
                              href={tool.github_url}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="flex items-center gap-1 text-blue-600 hover:text-blue-800"
                            >
                              <Github className="h-4 w-4" />
                              Repository
                            </a>
                          )}
                          {tool.codespace_url && (
                            <a 
                              href={tool.codespace_url}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="flex items-center gap-1 text-green-600 hover:text-green-800"
                            >
                              <Cloud className="h-4 w-4" />
                              Codespace
                            </a>
                          )}
                        </div>
                      </div>
                      
                      <button
                        onClick={() => handleDeleteTool(tool.id)}
                        className="text-red-600 hover:text-red-800 p-2"
                      >
                        <Trash2 className="h-4 w-4" />
                      </button>
                    </div>
                  </div>
                ))
              )}
            </div>
          )}
        </div>
      </div>
    </>
  );
}