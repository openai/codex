/**
 * API client for Tool Builder backend
 */

import axios from 'axios';

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8000';

const api = axios.create({
  baseURL: API_BASE_URL,
  timeout: 30000,
});

export interface ToolRequest {
  name: string;
  description: string;
  template?: string;
  features?: string[];
}

export interface Tool {
  id: string;
  name: string;
  description: string;
  status: string;
  created_at: string;
  github_url?: string;
  codespace_url?: string;
}

export interface ToolStatus {
  id: string;
  status: string;
  current_step: string;
  progress: number;
  logs: string[];
  error?: string;
}

export const generateTool = async (request: ToolRequest): Promise<Tool> => {
  const response = await api.post('/tools/create', request);
  return response.data;
};

export const getTools = async (skip = 0, limit = 20): Promise<Tool[]> => {
  const response = await api.get(`/tools?skip=${skip}&limit=${limit}`);
  return response.data;
};

export const getTool = async (toolId: string): Promise<Tool> => {
  const response = await api.get(`/tools/${toolId}`);
  return response.data;
};

export const getToolStatus = async (toolId: string): Promise<ToolStatus> => {
  const response = await api.get(`/tools/${toolId}/status`);
  return response.data;
};

export const deleteTool = async (toolId: string): Promise<void> => {
  await api.delete(`/tools/${toolId}`);
};

export const healthCheck = async () => {
  const response = await api.get('/health');
  return response.data;
};