import { defineRouteMiddleware } from '@astrojs/starlight/route-data';

// Starlight writes the content of a head entry as HTML. A page title is text,
// and one from an API reference can hold a template, such as Field<V>.
export const onRequest = defineRouteMiddleware(context => {
  for (const entry of context.locals.starlightRoute.head) {
    if (entry.tag === 'title' && entry.content) entry.content = escapeText(entry.content);
  }
});

function escapeText(text: string) {
  return text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
}
