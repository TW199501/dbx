import { fetchChangelog } from '@/lib/changelog';
import { LandingNav } from '@/components/landing/LandingNav';
import { ChangelogList } from '@/components/landing/ChangelogList';

const i18n = {
  en: {
    title: 'Changelog',
    desc: 'Track every release — features, improvements, and fixes.',
  },
  cn: {
    title: '更新日志',
    desc: '追踪每次发布 — 新功能、改进和修复。',
  },
};

export default async function ChangelogPage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params;
  const l = lang === 'cn' ? 'cn' : 'en';
  const t = i18n[l];
  const data = await fetchChangelog(l);

  return (
    <div className="min-h-screen bg-[#0b1120] text-landing-ink">
      <LandingNav lang={l} active="changelog" />

      <div className="max-w-[860px] mx-auto px-6 pt-32 pb-4">
        <h1 className="text-4xl font-[820] tracking-tight">{t.title}</h1>
        <p className="mt-3 text-landing-muted text-lg">{t.desc}</p>
      </div>

      <div className="max-w-[860px] mx-auto px-6 pb-24">
        {data.releases.length === 0 ? (
          <p className="text-landing-muted py-12">No releases found.</p>
        ) : (
          <ChangelogList releases={data.releases} lang={l} />
        )}
      </div>
    </div>
  );
}
