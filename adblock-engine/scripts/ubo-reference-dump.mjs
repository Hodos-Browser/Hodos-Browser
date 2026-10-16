// Reference conversion for the B5-T0-P6 differential test (`ubo_resources.rs ::
// test_ubo_conversion_matches_node`). Loads uBlock Origin's scriptlet modules the way uBO
// itself does — as ES modules, through its own registerScriptlet() — and prints every
// registration as uBO sees it: `fn.toString()` is exactly the text uBO injects.
//
// usage: node ubo-reference-dump.mjs <dir holding uBO's src/js tree>
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const root = process.argv[2];
const entry = pathToFileURL(path.join(root, 'resources', 'scriptlets.js')).href;
const { builtinScriptlets } = await import(entry);

const out = builtinScriptlets.map(s => ({
    name: s.name,
    aliases: s.aliases ?? [],
    code: s.fn.toString(),
    // A function left in here means uBO could not name it; stringify keeps it visible.
    dependencies: (s.dependencies ?? []).map(d => typeof d === 'string' ? d : `<unnamed ${typeof d}>`),
    requires_trust: s.requiresTrust === true,
    world: s.world ?? 'MAIN',
    priority: s.priority ?? 0,
}));
process.stdout.write(JSON.stringify(out));
