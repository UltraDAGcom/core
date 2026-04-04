/**
 * Shared theme constants for the new dashboard design.
 * All pages use inline styles with these values for consistency.
 *
 * Colors reference CSS custom properties defined in globalStyles below,
 * so they automatically adapt when `data-theme` is toggled on <html>.
 */

export const colors = {
  bg: 'var(--dag-bg)',
  card: 'var(--dag-card)',
  border: 'var(--dag-border)',
  borderHover: 'var(--dag-border-hover)',
  accent: '#00E0C4',
  blue: '#0066FF',
  purple: '#A855F7',
  yellow: '#FFB800',
  red: '#EF4444',
  green: '#00E0C4',
  textPrimary: 'var(--dag-text)',
  textSecondary: 'var(--dag-text-secondary)',
  textMuted: 'var(--dag-text-muted)',
  textFaint: 'var(--dag-text-faint)',
  sectionLabel: 'var(--dag-text-faint)',
} as const;

export const fonts = {
  sans: "'DM Sans',sans-serif",
  mono: "'DM Mono',monospace",
} as const;

export const cardStyle: React.CSSProperties = {
  background: colors.card,
  border: `1px solid ${colors.border}`,
  borderRadius: 14,
  padding: '16px 18px',
};

export const sectionLabelStyle: React.CSSProperties = {
  fontSize: 10,
  fontWeight: 600,
  color: colors.sectionLabel,
  letterSpacing: 2,
  textTransform: 'uppercase',
  marginBottom: 10,
};

export const pageStyle: React.CSSProperties = {
  padding: '18px 26px',
  fontFamily: fonts.sans,
};

export const headingStyle: React.CSSProperties = {
  fontSize: 21,
  fontWeight: 700,
  letterSpacing: -0.3,
  color: colors.textPrimary,
};

export const subheadingStyle: React.CSSProperties = {
  fontSize: 11.5,
  color: 'var(--dag-subheading)',
  marginTop: 2,
};

export const tableHeaderStyle: React.CSSProperties = {
  fontSize: 8.5,
  fontWeight: 600,
  color: colors.textFaint,
  letterSpacing: 1.5,
  paddingBottom: 7,
  borderBottom: '1px solid var(--dag-table-border)',
};

export const tableCellStyle: React.CSSProperties = {
  fontSize: 11.5,
  color: 'var(--dag-cell-text)',
  padding: '5px 0',
  borderBottom: '1px solid var(--dag-row-border)',
};

export const accentCellStyle: React.CSSProperties = {
  ...tableCellStyle,
  fontWeight: 600,
  color: colors.accent,
  fontFamily: fonts.mono,
};

export const buttonStyle = (accent = colors.accent): React.CSSProperties => ({
  padding: '8px 18px',
  borderRadius: 10,
  background: `${accent}15`,
  border: `1px solid ${accent}30`,
  color: accent,
  fontSize: 12.5,
  fontWeight: 600,
  cursor: 'pointer',
  transition: 'all 0.2s',
});

export const inputStyle: React.CSSProperties = {
  width: '100%',
  padding: '10px 14px',
  borderRadius: 10,
  background: 'var(--dag-input-bg)',
  border: `1px solid ${colors.border}`,
  color: colors.textPrimary,
  fontSize: 13,
  fontFamily: fonts.sans,
  outline: 'none',
};

export const globalStyles = `
  @import url('https://fonts.googleapis.com/css2?family=DM+Sans:wght@400;500;600;700&family=DM+Mono:wght@400;500&display=swap');
  @keyframes pulse{0%,100%{opacity:1}50%{opacity:.5}}
  @keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}}
  @keyframes glow{0%,100%{box-shadow:0 0 12px rgba(0,224,196,0.15)}50%{box-shadow:0 0 20px rgba(0,224,196,0.3)}}

  :root, [data-theme="dark"] {
    --dag-bg: #0d1117;
    --dag-card: rgba(255,255,255,0.035);
    --dag-card-hover: rgba(255,255,255,0.06);
    --dag-border: rgba(255,255,255,0.09);
    --dag-border-hover: rgba(255,255,255,0.18);
    --dag-text: #f0f2f5;
    --dag-text-secondary: rgba(255,255,255,0.7);
    --dag-text-muted: rgba(255,255,255,0.5);
    --dag-text-faint: rgba(255,255,255,0.3);
    --dag-input-bg: rgba(255,255,255,0.05);
    --dag-row-border: rgba(255,255,255,0.05);
    --dag-table-border: rgba(255,255,255,0.06);
    --dag-cell-text: rgba(255,255,255,0.6);
    --dag-value-text: rgba(255,255,255,0.8);
    --dag-subheading: rgba(255,255,255,0.4);
    --dag-sidebar-bg: rgba(255,255,255,0.015);
    --dag-sidebar-border: rgba(255,255,255,0.07);
    --dag-sidebar-section: rgba(255,255,255,0.2);
    --dag-sidebar-inactive: rgba(255,255,255,0.5);
    --dag-sidebar-lock-bg: rgba(255,255,255,0.03);
    --dag-sidebar-lock-border: rgba(255,255,255,0.07);
    --dag-sidebar-lock-text: rgba(255,255,255,0.45);
    --dag-sidebar-footer-border: rgba(255,255,255,0.06);
    --dag-sidebar-footer-text: rgba(255,255,255,0.45);
    --dag-sidebar-footer-muted: rgba(255,255,255,0.25);
    --dag-sidebar-footer-faint: rgba(255,255,255,0.18);
    --dag-overlay: rgba(0,0,0,0.6);
    --dag-net-inactive: rgba(255,255,255,0.3);
    --dag-net-switch-bg: rgba(255,255,255,0.03);
    --dag-net-switch-border: rgba(255,255,255,0.07);
  }

  [data-theme="light"] {
    --dag-bg: #F5F6F8;
    --dag-card: rgba(255,255,255,0.85);
    --dag-card-hover: rgba(0,0,0,0.02);
    --dag-border: rgba(0,0,0,0.08);
    --dag-border-hover: rgba(0,0,0,0.15);
    --dag-text: #1a1a2e;
    --dag-text-secondary: rgba(0,0,0,0.55);
    --dag-text-muted: rgba(0,0,0,0.35);
    --dag-text-faint: rgba(0,0,0,0.2);
    --dag-input-bg: rgba(0,0,0,0.03);
    --dag-row-border: rgba(0,0,0,0.06);
    --dag-table-border: rgba(0,0,0,0.06);
    --dag-cell-text: rgba(0,0,0,0.55);
    --dag-value-text: rgba(0,0,0,0.7);
    --dag-subheading: rgba(0,0,0,0.4);
    --dag-sidebar-bg: rgba(255,255,255,0.7);
    --dag-sidebar-border: rgba(0,0,0,0.06);
    --dag-sidebar-section: rgba(0,0,0,0.25);
    --dag-sidebar-inactive: rgba(0,0,0,0.45);
    --dag-sidebar-lock-bg: rgba(0,0,0,0.03);
    --dag-sidebar-lock-border: rgba(0,0,0,0.08);
    --dag-sidebar-lock-text: rgba(0,0,0,0.4);
    --dag-sidebar-footer-border: rgba(0,0,0,0.06);
    --dag-sidebar-footer-text: rgba(0,0,0,0.4);
    --dag-sidebar-footer-muted: rgba(0,0,0,0.25);
    --dag-sidebar-footer-faint: rgba(0,0,0,0.15);
    --dag-overlay: rgba(0,0,0,0.3);
    --dag-net-inactive: rgba(0,0,0,0.25);
    --dag-net-switch-bg: rgba(0,0,0,0.03);
    --dag-net-switch-border: rgba(0,0,0,0.08);
  }
`;

export const kvRowStyle: React.CSSProperties = {
  display: 'flex',
  justifyContent: 'space-between',
  padding: '6px 0',
  borderBottom: '1px solid var(--dag-row-border)',
};

export const kvLabelStyle: React.CSSProperties = {
  fontSize: 11.5,
  color: colors.textMuted,
};

export const kvValueStyle: React.CSSProperties = {
  fontSize: 11.5,
  fontWeight: 600,
  color: 'var(--dag-value-text)',
  fontFamily: fonts.mono,
};
