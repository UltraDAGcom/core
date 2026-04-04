import { createContext, useContext } from 'react';

export interface AppStatus {
  connected: boolean;
  network: string;
  userName: string;
  totalBalance: number;
  healthStatus: string | null;
  healthScore: number;
}

const AppStatusCtx = createContext<AppStatus | null>(null);

export function AppStatusProvider({ value, children }: { value: AppStatus; children: React.ReactNode }) {
  return <AppStatusCtx.Provider value={value}>{children}</AppStatusCtx.Provider>;
}

export function useAppStatus(): AppStatus | null {
  return useContext(AppStatusCtx);
}
