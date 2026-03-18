import { Construction } from 'lucide-react';

export function PlaceholderPage({ title, description }: { title: string; description: string }) {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">{title}</h1>
        <p className="text-sm text-dag-muted mt-1">{description}</p>
      </div>
      <div className="flex flex-col items-center justify-center py-20 space-y-4">
        <Construction className="w-12 h-12 text-slate-600" />
        <p className="text-dag-muted text-sm">This section is under construction.</p>
      </div>
    </div>
  );
}
