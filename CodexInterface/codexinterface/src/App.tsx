import React, { useState, useEffect } from 'react'
import CodexPage from './pages/CodexPage'
import SettingsPage from './pages/SettingsPage'
import Navbar from './components/Navbar'

function App() {
  const [activePage, setActivePage] = useState('codex')
  
  // Add dark mode class to document
  useEffect(() => {
    document.documentElement.classList.add('dark')
  }, [])
  
  // Handle page change
  const handlePageChange = (page: string) => {
    setActivePage(page)
  }
  
  // Render the active page
  const renderPage = () => {
    switch (activePage) {
      case 'codex':
        return <CodexPage />
      case 'settings':
        return <SettingsPage />
      default:
        return <CodexPage />
    }
  }

  return (
    <div className="min-h-screen bg-zinc-900 text-zinc-50">
      {/* Navigation */}
      <Navbar 
        activePage={activePage} 
        onPageChange={handlePageChange} 
      />
      
      {/* Page Content */}
      <div className="p-6">
        {renderPage()}
      </div>
    </div>
  )
}

export default App
