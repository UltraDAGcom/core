import rss from '@astrojs/rss';
import { getCollection } from 'astro:content';

export async function GET(context: { site: URL }) {
  const posts = await getCollection('blog');

  return rss({
    title: 'UltraDAG ($UDAG) Blog',
    description: 'Latest news, technical deep dives, and updates from the UltraDAG project.',
    site: context.site,
    items: posts
      .sort((a, b) => b.data.date.localeCompare(a.data.date))
      .map((post) => ({
        title: post.data.title,
        pubDate: new Date(post.data.date),
        description: post.data.summary,
        link: `/blog/${post.id.replace(/\.md$/, '')}/`,
        categories: [post.data.category],
      })),
    customData: '<language>en-us</language>',
  });
}
