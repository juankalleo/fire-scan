-- Create manga table
CREATE TABLE IF NOT EXISTS manga (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  source_name TEXT NOT NULL,
  source_url TEXT,
  cover_path TEXT,
  synopsis TEXT,
  status TEXT,
  rating REAL,
  language TEXT DEFAULT 'pt-BR',
  local_path TEXT NOT NULL UNIQUE,
  total_chapters INTEGER,
  downloaded_chapters INTEGER,
  last_updated TIMESTAMP,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create chapters table
CREATE TABLE IF NOT EXISTS chapters (
  id TEXT PRIMARY KEY,
  manga_id TEXT NOT NULL,
  chapter_number REAL NOT NULL,
  title TEXT,
  url TEXT,
  downloaded BOOLEAN DEFAULT 0,
  file_path TEXT,
  pages INTEGER,
  size_bytes INTEGER,
  released_at TIMESTAMP,
  FOREIGN KEY (manga_id) REFERENCES manga(id) ON DELETE CASCADE,
  UNIQUE(manga_id, chapter_number)
);

-- Create reading_progress table
CREATE TABLE IF NOT EXISTS reading_progress (
  id TEXT PRIMARY KEY,
  manga_id TEXT NOT NULL,
  chapter_id TEXT NOT NULL,
  current_page INTEGER,
  total_pages INTEGER,
  last_read TIMESTAMP,
  FOREIGN KEY (manga_id) REFERENCES manga(id) ON DELETE CASCADE,
  FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE CASCADE
);

-- Create favorites table
CREATE TABLE IF NOT EXISTS favorites (
  id TEXT PRIMARY KEY,
  manga_id TEXT NOT NULL UNIQUE,
  added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (manga_id) REFERENCES manga(id) ON DELETE CASCADE
);

-- Create downloads table
CREATE TABLE IF NOT EXISTS downloads (
  id TEXT PRIMARY KEY,
  manga_id TEXT NOT NULL,
  chapter_ids TEXT,
  status TEXT DEFAULT 'pending',
  progress_percent INTEGER DEFAULT 0,
  current_file TEXT,
  error_message TEXT,
  started_at TIMESTAMP,
  completed_at TIMESTAMP,
  FOREIGN KEY (manga_id) REFERENCES manga(id) ON DELETE CASCADE
);

-- Create sources table
CREATE TABLE IF NOT EXISTS sources (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  language TEXT,
  enabled BOOLEAN DEFAULT 1,
  priority INTEGER DEFAULT 999,
  last_synced TIMESTAMP
);

-- Create user_settings table
CREATE TABLE IF NOT EXISTS user_settings (
  key TEXT PRIMARY KEY,
  value TEXT,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create indices for performance
CREATE INDEX IF NOT EXISTS idx_manga_title ON manga(title);
CREATE INDEX IF NOT EXISTS idx_manga_source ON manga(source_name);
CREATE INDEX IF NOT EXISTS idx_chapters_manga ON chapters(manga_id);
CREATE INDEX IF NOT EXISTS idx_favorites_manga ON favorites(manga_id);
CREATE INDEX IF NOT EXISTS idx_downloads_status ON downloads(status);
