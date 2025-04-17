import React from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card'

const SettingsPage: React.FC = () => {
  return (
    <div className="p-6 h-[calc(100vh-4rem-3rem)]">
      <Card className="border border-zinc-800 shadow-[0_0_15px_rgba(0,0,0,0.5),inset_0_0_10px_rgba(92,124,250,0.1)] bg-zinc-800/40 backdrop-blur-sm overflow-hidden relative before:absolute before:inset-0 before:border-t before:border-zinc-700/30 before:rounded-lg">
        <CardHeader className="p-4 relative z-10">
          <CardTitle className="text-primary-foreground">Settings</CardTitle>
        </CardHeader>
        <CardContent className="p-4 pt-0 relative z-10">
          <p className="text-zinc-300">Settings page content will go here.</p>
        </CardContent>
      </Card>
    </div>
  )
}

export default SettingsPage
