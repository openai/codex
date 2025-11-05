import { useState, useCallback, useEffect } from 'react'
import './SearchBar.css'

export interface SearchFilters {
  searchText: string
  authors: string[]
  branches: string[]
  dateFrom: string
  dateTo: string
}

interface SearchBarProps {
  onFiltersChange: (filters: SearchFilters) => void
  availableAuthors: string[]
  availableBranches: string[]
}

export default function SearchBar({
  onFiltersChange,
  availableAuthors,
  availableBranches,
}: SearchBarProps) {
  const [isExpanded, setIsExpanded] = useState(false)
  const [searchText, setSearchText] = useState('')
  const [selectedAuthors, setSelectedAuthors] = useState<string[]>([])
  const [selectedBranches, setSelectedBranches] = useState<string[]>([])
  const [dateFrom, setDateFrom] = useState('')
  const [dateTo, setDateTo] = useState('')

  // Debounce search text
  useEffect(() => {
    const timer = setTimeout(() => {
      const filters: SearchFilters = {
        searchText,
        authors: selectedAuthors,
        branches: selectedBranches,
        dateFrom,
        dateTo,
      }
      onFiltersChange(filters)
    }, 300)

    return () => clearTimeout(timer)
  }, [searchText, selectedAuthors, selectedBranches, dateFrom, dateTo, onFiltersChange])

  const toggleAuthor = useCallback((author: string) => {
    setSelectedAuthors((prev) =>
      prev.includes(author)
        ? prev.filter((a) => a !== author)
        : [...prev, author]
    )
  }, [])

  const toggleBranch = useCallback((branch: string) => {
    setSelectedBranches((prev) =>
      prev.includes(branch)
        ? prev.filter((b) => b !== branch)
        : [...prev, branch]
    )
  }, [])

  const clearFilters = useCallback(() => {
    setSearchText('')
    setSelectedAuthors([])
    setSelectedBranches([])
    setDateFrom('')
    setDateTo('')
  }, [])

  const hasActiveFilters =
    searchText.length > 0 ||
    selectedAuthors.length > 0 ||
    selectedBranches.length > 0 ||
    dateFrom.length > 0 ||
    dateTo.length > 0

  return (
    <div className={`search-bar ${isExpanded ? 'expanded' : ''}`}>
      {/* Search input */}
      <div className="search-input-container">
        <input
          type="text"
          className="search-input"
          placeholder="Search commits, authors, messages..."
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          onFocus={() => setIsExpanded(true)}
        />
        <span className="search-icon">🔍</span>
        
        {hasActiveFilters && (
          <button
            className="clear-button"
            onClick={clearFilters}
            aria-label="Clear filters"
          >
            ✕
          </button>
        )}
      </div>

      {/* Expanded filters */}
      {isExpanded && (
        <div className="filters-panel">
          {/* Authors filter */}
          <div className="filter-group">
            <label className="filter-label">Authors</label>
            <div className="filter-chips">
              {availableAuthors.slice(0, 10).map((author) => (
                <button
                  key={author}
                  className={`filter-chip ${
                    selectedAuthors.includes(author) ? 'active' : ''
                  }`}
                  onClick={() => toggleAuthor(author)}
                >
                  {author}
                </button>
              ))}
            </div>
          </div>

          {/* Branches filter */}
          <div className="filter-group">
            <label className="filter-label">Branches</label>
            <div className="filter-chips">
              {availableBranches.map((branch) => (
                <button
                  key={branch}
                  className={`filter-chip ${
                    selectedBranches.includes(branch) ? 'active' : ''
                  }`}
                  onClick={() => toggleBranch(branch)}
                >
                  {branch}
                </button>
              ))}
            </div>
          </div>

          {/* Date range */}
          <div className="filter-group">
            <label className="filter-label">Date Range</label>
            <div className="date-inputs">
              <input
                type="date"
                className="date-input"
                value={dateFrom}
                onChange={(e) => setDateFrom(e.target.value)}
                placeholder="From"
              />
              <span className="date-separator">→</span>
              <input
                type="date"
                className="date-input"
                value={dateTo}
                onChange={(e) => setDateTo(e.target.value)}
                placeholder="To"
              />
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

