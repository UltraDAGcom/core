import { useIsMobile } from '../../hooks/useIsMobile';

interface PageHeaderProps {
  title: string;
  subtitle?: string;
  /** Right-side content (badges, buttons, etc.) */
  right?: React.ReactNode;
}

export function PageHeader({ title, subtitle, right }: PageHeaderProps) {
  const m = useIsMobile();

  return (
    <div style={{
      display: 'flex', justifyContent: 'space-between',
      alignItems: m ? 'flex-start' : 'center',
      marginBottom: m ? 16 : 22,
      flexDirection: m ? 'column' : 'row',
      gap: m ? 10 : 0,
    }}>
      <div>
        <h1 style={{ fontSize: m ? 18 : 21, fontWeight: 700, letterSpacing: -0.3, color: 'var(--dag-text)' }}>
          {title}
        </h1>
        {subtitle && (
          <p style={{ fontSize: 11.5, color: 'var(--dag-subheading)', marginTop: 2 }}>{subtitle}</p>
        )}
      </div>
      {right && (
        <div style={{ display: 'flex', alignItems: 'center', gap: m ? 8 : 12, flexWrap: 'wrap' }}>
          {right}
        </div>
      )}
    </div>
  );
}
