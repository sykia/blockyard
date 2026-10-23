const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');
const sharp = require('sharp');
const root = path.resolve(__dirname, '..');
(async () => {
  const browser = await chromium.launch({ headless: true, ...(process.env.BLOCKYARD_BROWSER ? {channel: process.env.BLOCKYARD_BROWSER} : {}) });
  const page = await browser.newPage({ viewport: { width: 1180, height: 760 }, deviceScaleFactor: 2 });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const mocks = fs.readFileSync(path.join(root, 'node_modules/@tauri-apps/api/mocks.js'), 'utf8').replace(/export \{[^}]+\};?/, '');
  await page.addInitScript({ content: mocks + `
    const sample = {
      instances: [
        {id:'vanilla',name:'Vanilla',version:'1.21.1',loader:{kind:'vanilla'},icon:'cube',ramMb:4096},
        {id:'fabric',name:'Fabric Essentials',version:'1.21.1',loader:{kind:'fabric',version:'0.16.10'},icon:'leaf',ramMb:6144},
        {id:'creative',name:'Creative Workshop',version:'1.21.1',loader:{kind:'vanilla'},icon:'pickaxe',ramMb:4096}
      ].map(i => ({javaPath:null,jvmArgs:'',width:1280,height:720,lastPlayed:null,playtimeSeconds:0,...i})),
      accounts:[{id:'sample',name:'Steve',kind:'offline',xuid:'',expiresAt:0}],activeAccount:'sample',warnings:[],
      settings:{ramMb:4096,javaPath:null,downloadConcurrency:8,showSnapshots:false,theme:'dark',animationsEnabled:false,minimizeOnLaunch:true,width:1280,height:720,microsoftClientId:''}
    };
    localStorage.setItem('blockyard-theme','dark');
    localStorage.setItem('blockyard-animations','off');
    mockIPC((cmd,args) => {
      if(cmd === 'snapshot') return structuredClone(sample);
      if(cmd === 'versions') return [{id:'1.21.1',kind:'release',releaseTime:'2024-08-08'}];
      if(cmd === 'save_settings') {sample.settings = args.settings;return null;}
      if(cmd === 'updater_ready' || cmd === 'catalog_key_status') return false;
      throw new Error('Unhandled screenshot command: ' + cmd);
    }, {shouldMockEvents:true});
    mockWindows('main');
  ` });
  await page.goto(process.env.BLOCKYARD_URL || 'http://localhost:1420');
  await page.getByRole('heading', {name:'Vanilla',exact:true}).waitFor();
  const capture = async name => {
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({path:path.join(root,'docs/screenshots',name+'.png')});
  };
  await capture('overview');
  await page.getByRole('button',{name:'White theme',exact:true}).click();
  await page.locator('html[data-theme="light"]').waitFor();
  await capture('overview-light');
  await page.getByRole('button',{name:'Black theme',exact:true}).click();
  await page.locator('html[data-theme="dark"]').waitFor();
  for (const [label,file] of [['Instances','instances'],['Accounts','accounts'],['Settings','settings']]) {
    await page.getByRole('navigation').getByRole('button',{name:label,exact:true}).click();
    await page.getByRole('heading',{name:label,exact:true,level:1}).waitFor();
    await capture(file);
  }
  // Verify compact navigation, appearance switching and modal fit at minimum desktop size.
  await page.setViewportSize({width:900,height:600});
  await page.getByRole('navigation').getByRole('button',{name:'Overview',exact:true}).click();
  await page.getByRole('button',{name:'New instance',exact:true}).click();
  if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)) throw new Error('Horizontal overflow at minimum desktop size');
  await page.screenshot({path:path.join(root,'docs/screenshots/create.png')});
  if (errors.length) throw new Error(errors.join('\n'));
  await browser.close();
  console.log('Logo assets and six screenshots generated; theme switching and compact layout verified.');
})().catch(error=>{console.error(error);process.exit(1);});

