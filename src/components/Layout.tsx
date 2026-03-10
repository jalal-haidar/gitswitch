import React from 'react';

interface LayoutProps {
  children: React.ReactNode;
}

export const Layout: React.FC<LayoutProps> = ({ children }) => {
  return (
    <div className="layout">
      <div className="bg-glow-1"></div>
      <div className="bg-glow-2"></div>
      <div className="container animate-fade-in">
        {children}
      </div>
    </div>
  );
};
