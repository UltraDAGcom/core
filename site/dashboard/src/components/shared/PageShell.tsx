import { pageStyle, headingStyle, subheadingStyle, globalStyles } from '../../lib/theme';

interface PageShellProps {
  title: string;
  subtitle?: string;
  children: React.ReactNode;
  action?: React.ReactNode;
}

export function PageShell({ title, subtitle, children, action }: PageShellProps) {
  return (
    <div style={pageStyle}>
      <style>{globalStyles}</style>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 22, animation: 'slideUp 0.3s ease' }}>
        <div>
          <h1 style={headingStyle}>{title}</h1>
          {subtitle && <p style={subheadingStyle}>{subtitle}</p>}
        </div>
        {action}
      </div>
      {children}
    </div>
  );
}
