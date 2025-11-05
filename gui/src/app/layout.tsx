import type { Metadata } from "next";
import localFont from "next/font/local";
import "./globals.css";
import { AppThemeProvider } from "@/components/templates/ThemeProvider";
import { PWAProvider } from "@/components/pwa/PWAProvider";
import { CodexProvider } from "@/lib/context/CodexContext";

const geistSans = localFont({
  src: "./fonts/GeistVF.woff",
  variable: "--font-geist-sans",
  weight: "100 900",
});
const geistMono = localFont({
  src: "./fonts/GeistMonoVF.woff",
  variable: "--font-geist-mono",
  weight: "100 900",
});

export const metadata: Metadata = {
  title: "Codex GUI - AI Assistant Platform",
  description: "Modern AI assistant platform with advanced sub-agents, deep research, and security features",
  keywords: ["AI", "assistant", "codex", "machine learning", "automation"],
  authors: [{ name: "zapabob" }],
  viewport: "width=device-width, initial-scale=1",
  manifest: "/manifest.json",
  appleWebApp: {
    capable: true,
    statusBarStyle: "default",
    title: "Codex GUI",
  },
  formatDetection: {
    telephone: false,
  },
  openGraph: {
    type: "website",
    siteName: "Codex GUI",
    title: "Codex GUI - AI Assistant Platform",
    description: "Modern AI assistant platform with advanced sub-agents, deep research, and security features",
  },
  twitter: {
    card: "summary_large_image",
    title: "Codex GUI - AI Assistant Platform",
    description: "Modern AI assistant platform with advanced sub-agents, deep research, and security features",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="ja" suppressHydrationWarning>
      <body
        className={`${geistSans.variable} ${geistMono.variable} antialiased`}
      >
        <PWAProvider>
          <CodexProvider>
            <AppThemeProvider>
              {children}
            </AppThemeProvider>
          </CodexProvider>
        </PWAProvider>
      </body>
    </html>
  );
}
