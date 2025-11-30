import { rm, mkdir, copyFile, readFile, writeFile } from 'node:fs/promises'

async function run () {
    await rm('dist', { force: true, recursive: true });
    await mkdir('dist');
    await copyFile('webpage/background.webp', 'dist/background.webp');

    let html = await readFile('webpage/index.html', 'utf8');
    let time = new Date().toISOString();
    let newHtml = html.replaceAll('{{time}}', time);
    await writeFile('dist/index.html', newHtml);

    await copyFile('united.fln', 'dist/united.fln');
    await copyFile('united.json', 'dist/united.json');
    await copyFile('united-lx.fln', 'dist/united-lx.fln');
}

run();
