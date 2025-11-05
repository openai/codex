-- Prism Database Schema
-- Supabase PostgreSQL
-- Version: 1.0.0
-- Updated: 2025-11-02

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ========================================
-- 1. Profiles (User metadata)
-- ========================================
CREATE TABLE profiles (
  id UUID PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
  email TEXT UNIQUE NOT NULL,
  display_name TEXT,
  avatar_url TEXT,
  github_username TEXT,
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- RLS Policies for profiles
ALTER TABLE profiles ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can view own profile"
  ON profiles FOR SELECT
  USING (auth.uid() = id);

CREATE POLICY "Users can update own profile"
  ON profiles FOR UPDATE
  USING (auth.uid() = id);

CREATE POLICY "Users can insert own profile"
  ON profiles FOR INSERT
  WITH CHECK (auth.uid() = id);

-- ========================================
-- 2. User API Keys (Encrypted)
-- ========================================
CREATE TABLE user_api_keys (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  user_id UUID NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  provider TEXT NOT NULL CHECK (provider IN ('openai', 'anthropic')),
  encrypted_key TEXT NOT NULL,
  key_name TEXT, -- User-defined name for the key
  is_active BOOLEAN DEFAULT true,
  last_used_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW(),
  UNIQUE(user_id, provider, key_name)
);

CREATE INDEX idx_user_api_keys_user_provider ON user_api_keys(user_id, provider);

-- RLS Policies
ALTER TABLE user_api_keys ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can manage own API keys"
  ON user_api_keys FOR ALL
  USING (auth.uid() = user_id);

-- ========================================
-- 3. Repositories
-- ========================================
CREATE TABLE repositories (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  user_id UUID NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  repo_url TEXT NOT NULL,
  repo_name TEXT NOT NULL,
  repo_owner TEXT, -- GitHub username/org
  default_branch TEXT DEFAULT 'main',
  last_analyzed_at TIMESTAMPTZ,
  total_commits INTEGER DEFAULT 0,
  total_files INTEGER DEFAULT 0,
  total_lines INTEGER DEFAULT 0,
  primary_language TEXT,
  languages JSONB, -- {"Rust": 65.4, "TypeScript": 25.2, ...}
  storage_path TEXT, -- Supabase Storage path
  is_public BOOLEAN DEFAULT false,
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW(),
  UNIQUE(user_id, repo_url)
);

CREATE INDEX idx_repositories_user ON repositories(user_id);
CREATE INDEX idx_repositories_public ON repositories(is_public) WHERE is_public = true;

-- RLS Policies
ALTER TABLE repositories ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can manage own repositories"
  ON repositories FOR ALL
  USING (auth.uid() = user_id);

CREATE POLICY "Anyone can view public repositories"
  ON repositories FOR SELECT
  USING (is_public = true);

-- ========================================
-- 4. Visualizations
-- ========================================
CREATE TABLE visualizations (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  repository_id UUID NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  title TEXT,
  description TEXT,
  visualization_type TEXT DEFAULT '3d' CHECK (visualization_type IN ('3d', '4d', 'heatmap', 'network')),
  
  -- Metadata (stored in DB)
  metadata JSONB, -- {total_commits, date_range, etc}
  
  -- Large data (stored in Supabase Storage)
  data_storage_path TEXT, -- nodes, edges, timeline data
  
  -- Sharing
  share_token TEXT UNIQUE DEFAULT encode(gen_random_bytes(16), 'hex'),
  is_public BOOLEAN DEFAULT false,
  share_expires_at TIMESTAMPTZ,
  
  -- Stats
  view_count INTEGER DEFAULT 0,
  
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_visualizations_repo ON visualizations(repository_id);
CREATE INDEX idx_visualizations_user ON visualizations(user_id);
CREATE INDEX idx_visualizations_share_token ON visualizations(share_token);

-- RLS Policies
ALTER TABLE visualizations ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can manage own visualizations"
  ON visualizations FOR ALL
  USING (auth.uid() = user_id);

CREATE POLICY "Anyone can view public visualizations"
  ON visualizations FOR SELECT
  USING (is_public = true);

CREATE POLICY "Anyone with share token can view"
  ON visualizations FOR SELECT
  USING (
    share_token IS NOT NULL AND
    (share_expires_at IS NULL OR share_expires_at > NOW())
  );

-- ========================================
-- 5. AI Sessions (Chat history)
-- ========================================
CREATE TABLE ai_sessions (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  user_id UUID NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  repository_id UUID REFERENCES repositories(id) ON DELETE SET NULL,
  
  -- AI Provider info
  provider TEXT NOT NULL CHECK (provider IN ('openai', 'anthropic')),
  model TEXT NOT NULL,
  
  -- Conversation
  title TEXT,
  messages JSONB NOT NULL DEFAULT '[]'::jsonb, -- [{role, content, timestamp}]
  
  -- Metrics
  total_tokens INTEGER DEFAULT 0,
  total_messages INTEGER DEFAULT 0,
  
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_ai_sessions_user ON ai_sessions(user_id);
CREATE INDEX idx_ai_sessions_repo ON ai_sessions(repository_id);
CREATE INDEX idx_ai_sessions_created ON ai_sessions(created_at DESC);

-- RLS Policies
ALTER TABLE ai_sessions ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can manage own sessions"
  ON ai_sessions FOR ALL
  USING (auth.uid() = user_id);

-- ========================================
-- 6. Usage Stats (将来の課金用)
-- ========================================
CREATE TABLE usage_stats (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  user_id UUID NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  
  -- Provider & Model
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  
  -- Usage metrics
  tokens_used INTEGER NOT NULL,
  request_count INTEGER DEFAULT 1,
  
  -- Cost tracking (for future billing)
  estimated_cost_usd DECIMAL(10, 6),
  
  -- Timestamps
  created_at TIMESTAMPTZ DEFAULT NOW(),
  period_start DATE GENERATED ALWAYS AS (DATE(created_at)) STORED
);

CREATE INDEX idx_usage_stats_user_period ON usage_stats(user_id, period_start);
CREATE INDEX idx_usage_stats_provider ON usage_stats(provider, model);

-- RLS Policies
ALTER TABLE usage_stats ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can view own usage stats"
  ON usage_stats FOR SELECT
  USING (auth.uid() = user_id);

CREATE POLICY "System can insert usage stats"
  ON usage_stats FOR INSERT
  WITH CHECK (auth.uid() = user_id);

-- ========================================
-- 7. Comments (Collaboration)
-- ========================================
CREATE TABLE comments (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  visualization_id UUID NOT NULL REFERENCES visualizations(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  commit_sha TEXT NOT NULL,
  content TEXT NOT NULL,
  
  -- Metadata
  position JSONB, -- {x, y, z} for 3D position
  
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_comments_visualization ON comments(visualization_id);
CREATE INDEX idx_comments_commit ON comments(commit_sha);

-- RLS Policies
ALTER TABLE comments ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can create comments"
  ON comments FOR INSERT
  WITH CHECK (auth.uid() = user_id);

CREATE POLICY "Users can update own comments"
  ON comments FOR UPDATE
  USING (auth.uid() = user_id);

CREATE POLICY "Users can delete own comments"
  ON comments FOR DELETE
  USING (auth.uid() = user_id);

CREATE POLICY "Anyone can view comments on public visualizations"
  ON comments FOR SELECT
  USING (
    EXISTS (
      SELECT 1 FROM visualizations v
      WHERE v.id = comments.visualization_id
      AND (v.is_public = true OR v.user_id = auth.uid())
    )
  );

-- ========================================
-- 8. Functions
-- ========================================

-- Update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ language 'plpgsql';

-- Apply to all tables
CREATE TRIGGER update_profiles_updated_at BEFORE UPDATE ON profiles
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_user_api_keys_updated_at BEFORE UPDATE ON user_api_keys
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_repositories_updated_at BEFORE UPDATE ON repositories
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_visualizations_updated_at BEFORE UPDATE ON visualizations
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_ai_sessions_updated_at BEFORE UPDATE ON ai_sessions
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_comments_updated_at BEFORE UPDATE ON comments
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ========================================
-- 9. Views (便利なクエリ)
-- ========================================

-- User usage summary
CREATE VIEW user_usage_summary AS
SELECT 
  user_id,
  DATE_TRUNC('month', created_at) as month,
  provider,
  SUM(tokens_used) as total_tokens,
  SUM(request_count) as total_requests,
  SUM(estimated_cost_usd) as total_cost
FROM usage_stats
GROUP BY user_id, DATE_TRUNC('month', created_at), provider;

-- Repository statistics
CREATE VIEW repository_stats AS
SELECT 
  r.id,
  r.repo_name,
  r.user_id,
  r.total_commits,
  COUNT(DISTINCT v.id) as visualization_count,
  SUM(v.view_count) as total_views,
  MAX(v.created_at) as last_visualization_at
FROM repositories r
LEFT JOIN visualizations v ON v.repository_id = r.id
GROUP BY r.id, r.repo_name, r.user_id, r.total_commits;

-- ========================================
-- 10. Initial Data (Optional)
-- ========================================

-- Example: Create system user for demos
-- INSERT INTO auth.users (id, email) VALUES 
--   ('00000000-0000-0000-0000-000000000000', 'demo@prism.dev');

-- ========================================
-- 11. Storage Policies
-- ========================================

-- Note: Storage policies are set via Supabase Dashboard
-- Buckets to create:
-- 1. visualizations (public read)
-- 2. avatars (public read)
-- 3. artifacts (private)

-- ========================================
-- 12. Realtime Publication (Optional)
-- ========================================

-- Enable realtime for specific tables
ALTER PUBLICATION supabase_realtime ADD TABLE visualizations;
ALTER PUBLICATION supabase_realtime ADD TABLE comments;

-- ========================================
-- END OF SCHEMA
-- ========================================

-- Apply this schema in Supabase:
-- 1. Dashboard → SQL Editor
-- 2. Paste this entire file
-- 3. Run

