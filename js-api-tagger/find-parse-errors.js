#!/usr/bin/env node
import * as cheerio from 'cheerio';
import * as acorn from 'acorn';
import { readFileSync, readdirSync } from 'fs';
import { join, basename } from 'path';

const toolsDir = '/tmp/tools';
const htmlFiles = readdirSync(toolsDir)
  .filter(f => f.endsWith('.html'))
  .map(f => join(toolsDir, f));

for (const file of htmlFiles) {
  const html = readFileSync(file, 'utf-8');
  const $ = cheerio.load(html);

  $('script').each((_, el) => {
    const scriptEl = $(el);
    const src = scriptEl.attr('src');
    if (src) return;

    const code = scriptEl.text().trim();
    if (!code) return;

    const isModule = scriptEl.attr('type') === 'module';
    try {
      acorn.parse(code, {
        ecmaVersion: 'latest',
        sourceType: isModule ? 'module' : 'script',
        allowHashBang: true,
        allowAwaitOutsideFunction: true,
        allowImportExportEverywhere: true,
      });
    } catch (e) {
      console.log(`${basename(file)}: ${e.message}`);
      // Show context around error
      const lines = code.split('\n');
      const errorLine = e.loc?.line;
      if (errorLine && lines[errorLine - 1]) {
        console.log(`  Line ${errorLine}: ${lines[errorLine - 1].substring(0, 80)}`);
      }
    }
  });
}
