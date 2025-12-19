import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
    title: "Docker Chrome Neko",
    description: "Remote browser control with neko + CDP",
};

export default function RootLayout({
    children,
}: Readonly<{
    children: React.ReactNode;
}>) {
    return (
        <html lang="en">
            <body className="antialiased">
                {children}
            </body>
        </html>
    );
}
