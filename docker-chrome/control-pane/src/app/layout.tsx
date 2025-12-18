import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Chrome Control Pane",
  description: "Remote control for Docker Chrome",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className="antialiased min-h-screen bg-background">
        {children}
      </body>
    </html>
  );
}
