(function(){
'use strict';

// ── Elements ──────────────────────────────────────────────────────────────
const canvas   = document.getElementById('scene');
const ctx      = canvas.getContext('2d');
const hintEl   = document.getElementById('hint');
const finEl    = document.getElementById('fin');
const stepEl   = document.getElementById('step');
const finMs    = document.getElementById('fin-ms');
const finHops  = document.getElementById('fin-hops');
const finRound = document.getElementById('fin-round');
const finBtn   = document.getElementById('fin-btn');
const sd1=document.getElementById('sd1'),st1=document.getElementById('st1');
const sd2=document.getElementById('sd2'),st2=document.getElementById('st2');

// ── Config ────────────────────────────────────────────────────────────────
const CELL    = 48;
const N_AMB   = 16;
const A_SPEED = 0.0045;

// colours as [r,g,b]
const COL = {
  accent:  [61,155,233],
  success: [34,211,160],
  packet:  [255,208,70],
  warn:    [245,158,11],
  muted:   [140,172,210],
};

let W,H,COLS,ROWS;

// ── Utils ─────────────────────────────────────────────────────────────────
const clampC = c=>Math.max(0,Math.min(COLS-1,c));
const clampR = r=>Math.max(0,Math.min(ROWS-1,r));
const ease   = t=>t*t*(3-2*t);
const easeIO = t=>t<.5?2*t*t:1-Math.pow(-2*t+2,2)/2;
const easeO  = t=>1-(1-t)*(1-t);
const lerp   = (a,b,t)=>a+(b-a)*t;
const rgb    = ([r,g,b],a=1)=>`rgba(${r},${g},${b},${a})`;
function shuffle(a){for(let i=a.length-1;i>0;i--){const j=Math.floor(Math.random()*(i+1));[a[i],a[j]]=[a[j],a[i]]}}
function rrect(x,y,w,h,r=2){
  ctx.beginPath();
  ctx.moveTo(x+r,y);ctx.lineTo(x+w-r,y);ctx.arcTo(x+w,y,x+w,y+r,r);
  ctx.lineTo(x+w,y+h-r);ctx.arcTo(x+w,y+h,x+w-r,y+h,r);
  ctx.lineTo(x+r,y+h);ctx.arcTo(x,y+h,x,y+h-r,r);
  ctx.lineTo(x,y+r);ctx.arcTo(x,y,x+r,y,r);ctx.closePath();
}
function rHex(n){return[...Array(n)].map(()=>Math.floor(Math.random()*16).toString(16)).join('')}

// ── Ambient nodes ─────────────────────────────────────────────────────────
const ants=[];

function resize(){
  W=canvas.width=window.innerWidth;
  H=canvas.height=window.innerHeight;
  COLS=Math.ceil(W/CELL)+1;
  ROWS=Math.ceil(H/CELL)+1;
}

function initAmbient(){
  ants.length=0;
  const dirs=[{dx:1,dy:0},{dx:-1,dy:0},{dx:0,dy:1},{dx:0,dy:-1}];
  for(let i=0;i<N_AMB;i++){
    const col=Math.floor(Math.random()*COLS);
    const row=Math.floor(Math.random()*ROWS);
    const d=dirs[Math.floor(Math.random()*4)];
    ants.push({
      col,row,
      toCol:clampC(col+d.dx),toRow:clampR(row+d.dy),
      t:Math.random(),
      speed:A_SPEED*(0.4+Math.random()*1.2),
      size:1.6+Math.random()*1.8,
      bright:0.3+Math.random()*0.5,
      fin:Math.random()<0.14,
      label:null,
    });
  }
}

function nextAnt(n){
  const dirs=[{dx:1,dy:0},{dx:-1,dy:0},{dx:0,dy:1},{dx:0,dy:-1}];
  shuffle(dirs);
  for(const {dx,dy} of dirs){
    const nc=clampC(n.col+dx),nr=clampR(n.row+dy);
    if(nc!==n.col||nr!==n.row){
      n.toCol=nc;n.toRow=nr;n.t=0;
      if(Math.random()<.07) n.fin=!n.fin;
      return;
    }
  }
}

// ── Labels ────────────────────────────────────────────────────────────────
let txC=14832+Math.floor(Math.random()*300);
let rnC=14209+Math.floor(Math.random()*60);
let fnC=rnC-2;

const LCOL={tx:COL.accent,fin:COL.success,rnd:COL.muted,val:COL.warn};
const LEVENTS=[
  ()=>({k:'tx',  l1:`tx #${(++txC).toLocaleString()}`,l2:`${(Math.random()*4+.1).toFixed(3)} UDAG`}),
  ()=>({k:'fin', l1:`finalized`,                       l2:`round ${(++fnC).toLocaleString()}`}),
  ()=>({k:'rnd', l1:`round ${(++rnC).toLocaleString()}`,l2:`4 vertices`}),
  ()=>({k:'val', l1:`validator online`,                 l2:`stake 10k UDAG`}),
  ()=>({k:'val', l1:`checkpoint`,                       l2:`quorum 3/4`}),
  ()=>({k:'val', l1:`block reward`,                     l2:`+50 UDAG`}),
  ()=>({k:'val', l1:`peer connected`,                   l2:`${rHex(4)}:9333`}),
];

function spawnLabel(){
  const free=ants.filter(n=>!n.label&&n.col*CELL>W*.35);
  if(!free.length) return;
  const n=free[Math.floor(Math.random()*free.length)];
  const fn=LEVENTS[Math.floor(Math.random()*LEVENTS.length)];
  n.label={...fn(),age:0,fi:20,hold:120+Math.floor(Math.random()*80),fo:22,alpha:0};
}

function tickLabel(n){
  const lb=n.label;if(!lb) return;
  lb.age++;
  const tot=lb.fi+lb.hold+lb.fo;
  if(lb.age<=lb.fi)              lb.alpha=easeO(lb.age/lb.fi);
  else if(lb.age<=lb.fi+lb.hold) lb.alpha=1;
  else if(lb.age<tot)            lb.alpha=1-easeO((lb.age-lb.fi-lb.hold)/lb.fo);
  else n.label=null;
}

function drawLabel(px,py,lb){
  if(!lb||lb.alpha<.01) return;
  const col=LCOL[lb.k]||COL.muted;
  const a=lb.alpha;

  ctx.font='bold 8.5px "DM Mono",monospace';
  const tw1=ctx.measureText(lb.l1).width;
  ctx.font='8px "DM Mono",monospace';
  const tw2=ctx.measureText(lb.l2).width;
  const bw=Math.max(tw1,tw2)+16;
  const bh=25;

  let bx=px+11,by=py-bh-10;
  if(bx+bw>W-10) bx=px-bw-11;
  if(by<6)        by=py+12;

  // subtle drop shadow
  ctx.shadowColor=rgb(col,.12*a);
  ctx.shadowBlur=8;

  ctx.fillStyle=`rgba(4,8,16,${.92*a})`;
  rrect(bx,by,bw,bh); ctx.fill();
  ctx.shadowBlur=0;

  ctx.strokeStyle=rgb(col,.3*a);
  ctx.lineWidth=.8;
  rrect(bx,by,bw,bh); ctx.stroke();

  // left bar
  ctx.fillStyle=rgb(col,.7*a);
  rrect(bx,by,1.5,bh,1); ctx.fill();

  ctx.textBaseline='top';
  ctx.fillStyle=rgb(col,a);
  ctx.font='bold 8.5px "DM Mono",monospace';
  ctx.fillText(lb.l1,bx+8,by+4);
  ctx.fillStyle=`rgba(100,140,180,${.7*a})`;
  ctx.font='8px "DM Mono",monospace';
  ctx.fillText(lb.l2,bx+8,by+15);
}

// ── Game ──────────────────────────────────────────────────────────────────
// state: idle | placing | routing | done
let gState='idle';
let gSrc=null,gDst=null,gPath=[],gPkt=null;
let gStartMs=0;
let hCol=-1,hRow=-1;
const trail=[];  // [{col,row,age,alpha}]

// particle burst on finalize
const particles=[];

function snap(mx,my){
  return{col:clampC(Math.round(mx/CELL)),row:clampR(Math.round(my/CELL))};
}

// BFS
function bfs(sc,sr,dc,dr){
  if(sc===dc&&sr===dr) return [{col:sc,row:sr}];
  const key=(c,r)=>`${c},${r}`;
  const vis=new Set([key(sc,sr)]);
  const q=[{col:sc,row:sr,path:[{col:sc,row:sr}]}];
  const dirs=[{dx:1,dy:0},{dx:-1,dy:0},{dx:0,dy:1},{dx:0,dy:-1}];
  while(q.length){
    const cur=q.shift();
    for(const{dx,dy}of dirs){
      const nc=clampC(cur.col+dx),nr=clampR(cur.row+dy);
      const k=key(nc,nr);
      if(vis.has(k)) continue;
      vis.add(k);
      const np=[...cur.path,{col:nc,row:nr}];
      if(nc===dc&&nr===dr) return np;
      q.push({col:nc,row:nr,path:np});
    }
  }
  return [{col:sc,row:sr}];
}

function handleClick(mx,my){
  if(finEl.classList.contains('show')) return;
  const{col,row}=snap(mx,my);

  if(gState==='idle'||gState==='done'){
    // reset
    trail.length=0;particles.length=0;gPkt=null;gPath=[];gDst=null;
    gSrc={col,row};
    gState='placing';
    hintEl.classList.add('gone');
    stepEl.classList.add('show');
    // update step UI
    sd1.style.background='#f5d050';st1.className='step-txt active';
    sd2.style.background='#1a2e4a';st2.className='step-txt';
  } else if(gState==='placing'){
    if(col===gSrc.col&&row===gSrc.row) return;
    gDst={col,row};
    gPath=bfs(gSrc.col,gSrc.row,gDst.col,gDst.row);
    if(gPath.length<2) return;
    gPkt={
      idx:0,
      col:gPath[0].col,row:gPath[0].row,
      toCol:gPath[1].col,toRow:gPath[1].row,
      t:0,speed:0.048,
    };
    gStartMs=performance.now();
    gState='routing';
    // step UI
    sd1.style.background='#22d3a0';st1.className='step-txt done';
    sd2.style.background='#22d3a0';st2.className='step-txt done';
    setTimeout(()=>stepEl.classList.remove('show'),800);
  }
}

function tickPacket(){
  if(!gPkt) return;
  gPkt.t+=gPkt.speed;
  if(gPkt.t>=1){
    gPkt.col=gPkt.toCol;gPkt.row=gPkt.toRow;
    trail.push({col:gPkt.col,row:gPkt.row,age:0});
    gPkt.idx++;
    if(gPkt.idx>=gPath.length-1){
      burstParticles(gPkt.col*CELL,gPkt.row*CELL);
      arrived();
      return;
    }
    gPkt.toCol=gPath[gPkt.idx+1].col;
    gPkt.toRow=gPath[gPkt.idx+1].row;
    gPkt.t=0;
  }
}

function burstParticles(px,py){
  for(let i=0;i<28;i++){
    const ang=Math.random()*Math.PI*2;
    const spd=1.2+Math.random()*3.5;
    particles.push({
      x:px,y:py,
      vx:Math.cos(ang)*spd,vy:Math.sin(ang)*spd,
      life:1,decay:.025+Math.random()*.02,
      size:1+Math.random()*2.5,
      col:Math.random()<.6?COL.success:COL.packet,
    });
  }
}

function tickParticles(){
  for(let i=particles.length-1;i>=0;i--){
    const p=particles[i];
    p.x+=p.vx;p.y+=p.vy;
    p.vy+=.06; // gentle gravity
    p.life-=p.decay;
    if(p.life<=0) particles.splice(i,1);
  }
}

function arrived(){
  gState='done';
  const ms=Math.round(performance.now()-gStartMs);
  const hops=gPath.length-1;
  finMs.textContent=ms;
  finHops.textContent=hops;
  finRound.textContent=(rnC+1).toLocaleString();
  setTimeout(()=>finEl.classList.add('show'),400);
  gPkt=null;
}

finBtn.addEventListener('click',()=>{
  finEl.classList.remove('show');
  gState='idle';gSrc=null;gDst=null;gPath=[];
  trail.length=0;particles.length=0;
  stepEl.classList.remove('show');
});

// ── Draw game ─────────────────────────────────────────────────────────────
function drawGame(){
  if(gState==='idle') return;

  // trail
  for(let i=trail.length-1;i>=0;i--){
    const tr=trail[i];
    tr.age+=.8;
    if(tr.age>55){trail.splice(i,1);continue}
    const a=(1-tr.age/55)*.45;
    ctx.fillStyle=rgb(COL.packet,a);
    ctx.beginPath();ctx.arc(tr.col*CELL,tr.row*CELL,2.5,0,Math.PI*2);ctx.fill();
  }

  // planned path (dashed, very faint)
  if(gPath.length>1){
    ctx.save();
    ctx.strokeStyle=rgb(COL.packet,.13);
    ctx.lineWidth=1.2;
    ctx.setLineDash([3,7]);
    ctx.beginPath();
    ctx.moveTo(gPath[0].col*CELL,gPath[0].row*CELL);
    for(let i=1;i<gPath.length;i++) ctx.lineTo(gPath[i].col*CELL,gPath[i].row*CELL);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.restore();
  }

  // source
  if(gSrc){
    const sx=gSrc.col*CELL,sy=gSrc.row*CELL;
    const t=performance.now()/1000;
    const pulse=.55+.45*Math.sin(t*3.2);

    ctx.shadowColor=rgb(COL.packet,.4);ctx.shadowBlur=18;
    const g=ctx.createRadialGradient(sx,sy,0,sx,sy,26*pulse);
    g.addColorStop(0,rgb(COL.packet,.22));g.addColorStop(1,rgb(COL.packet,0));
    ctx.fillStyle=g;ctx.beginPath();ctx.arc(sx,sy,26*pulse,0,Math.PI*2);ctx.fill();

    ctx.strokeStyle=rgb(COL.packet,.85);ctx.lineWidth=1.5;
    ctx.beginPath();ctx.arc(sx,sy,6,0,Math.PI*2);ctx.stroke();
    ctx.fillStyle=rgb(COL.packet,1);
    ctx.beginPath();ctx.arc(sx,sy,3,0,Math.PI*2);ctx.fill();
    ctx.shadowBlur=0;

    // label
    ctx.font='bold 8px "DM Mono",monospace';
    ctx.fillStyle=rgb(COL.packet,.6);ctx.textBaseline='middle';
    ctx.fillText('SOURCE',sx+11,sy-14);
  }

  // destination
  if(gDst){
    const dx=gDst.col*CELL,dy=gDst.row*CELL;
    const t=performance.now()/1000;
    const pulse=.55+.45*Math.sin(t*2.8+1.2);
    const col=gState==='done'?COL.success:COL.success;

    ctx.shadowColor=rgb(col,.35);ctx.shadowBlur=16;
    const g=ctx.createRadialGradient(dx,dy,0,dx,dy,24*pulse);
    g.addColorStop(0,rgb(col,.2));g.addColorStop(1,rgb(col,0));
    ctx.fillStyle=g;ctx.beginPath();ctx.arc(dx,dy,24*pulse,0,Math.PI*2);ctx.fill();

    ctx.strokeStyle=rgb(col,.8);ctx.lineWidth=1.5;
    ctx.beginPath();ctx.arc(dx,dy,6,0,Math.PI*2);ctx.stroke();
    ctx.fillStyle=rgb(col,.9);
    ctx.beginPath();ctx.arc(dx,dy,3,0,Math.PI*2);ctx.fill();
    ctx.shadowBlur=0;

    ctx.font='bold 8px "DM Mono",monospace';
    ctx.fillStyle=rgb(col,.6);ctx.textBaseline='middle';
    ctx.fillText('DEST',dx+11,dy-14);
  }

  // hover crosshair (only when placing)
  if(gState==='placing'&&hCol>=0){
    const hx=hCol*CELL,hy=hRow*CELL;
    ctx.save();
    ctx.strokeStyle=rgb(COL.packet,.28);
    ctx.lineWidth=1;ctx.setLineDash([2,5]);
    ctx.beginPath();ctx.moveTo(hx-14,hy);ctx.lineTo(hx+14,hy);ctx.stroke();
    ctx.beginPath();ctx.moveTo(hx,hy-14);ctx.lineTo(hx,hy+14);ctx.stroke();
    ctx.setLineDash([]);ctx.restore();
  }

  // packet
  if(gPkt){
    const et=easeIO(gPkt.t);
    const px=lerp(gPkt.col,gPkt.toCol,et)*CELL;
    const py=lerp(gPkt.row,gPkt.toRow,et)*CELL;
    const prog=gPkt.idx/(gPath.length-1);

    // outer bloom
    const g=ctx.createRadialGradient(px,py,0,px,py,32);
    g.addColorStop(0,rgb(COL.packet,.5));
    g.addColorStop(.35,rgb(COL.packet,.18));
    g.addColorStop(1,rgb(COL.packet,0));
    ctx.fillStyle=g;ctx.beginPath();ctx.arc(px,py,32,0,Math.PI*2);ctx.fill();

    // core with glow
    ctx.shadowColor=rgb(COL.packet,.95);ctx.shadowBlur=18;
    ctx.fillStyle='rgba(255,230,130,1)';
    ctx.beginPath();ctx.arc(px,py,4.5,0,Math.PI*2);ctx.fill();
    ctx.shadowBlur=0;

    // progress %
    const pct=Math.round(prog*100);
    ctx.font='bold 8px "DM Mono",monospace';
    ctx.fillStyle=rgb(COL.packet,.75);ctx.textBaseline='middle';
    ctx.fillText(`${pct}%`,px+10,py-13);
  }

  // particles
  for(const p of particles){
    ctx.globalAlpha=p.life;
    ctx.fillStyle=rgb(p.col,1);
    ctx.shadowColor=rgb(p.col,.8);ctx.shadowBlur=6;
    ctx.beginPath();ctx.arc(p.x,p.y,p.size,0,Math.PI*2);ctx.fill();
    ctx.shadowBlur=0;
  }
  ctx.globalAlpha=1;
}

// ── Draw grid ─────────────────────────────────────────────────────────────
function drawGrid(){
  ctx.strokeStyle='rgba(18,30,52,0.78)';
  ctx.lineWidth=1;
  ctx.beginPath();
  for(let x=0;x<=W;x+=CELL){ctx.moveTo(x,0);ctx.lineTo(x,H)}
  for(let y=0;y<=H;y+=CELL){ctx.moveTo(0,y);ctx.lineTo(W,y)}
  ctx.stroke();
}

// ── Draw ambient edges ────────────────────────────────────────────────────
function drawEdges(){
  for(let i=0;i<ants.length;i++){
    for(let j=i+1;j<ants.length;j++){
      const a=ants[i],b=ants[j];
      const d=Math.abs(a.col-b.col)+Math.abs(a.row-b.row);
      if(d>2) continue;
      const ax=(a.col+(a.toCol-a.col)*ease(a.t))*CELL;
      const ay=(a.row+(a.toRow-a.row)*ease(a.t))*CELL;
      const bx=(b.col+(b.toCol-b.col)*ease(b.t))*CELL;
      const by=(b.row+(b.toRow-b.row)*ease(b.t))*CELL;
      ctx.strokeStyle=`rgba(61,155,233,${.06*(1-d/3)})`;
      ctx.lineWidth=1;
      ctx.beginPath();ctx.moveTo(ax,ay);ctx.lineTo(bx,by);ctx.stroke();
    }
  }
}

// ── Draw ambient nodes ────────────────────────────────────────────────────
function drawAmbient(){
  for(const n of ants){
    const et=ease(n.t);
    const px=(n.col+(n.toCol-n.col)*et)*CELL;
    const py=(n.row+(n.toRow-n.row)*et)*CELL;
    const col=n.fin?COL.success:COL.accent;
    const [r,g,b]=col;

    const grd=ctx.createRadialGradient(px,py,0,px,py,14);
    grd.addColorStop(0,`rgba(${r},${g},${b},${.24*n.bright})`);
    grd.addColorStop(1,`rgba(${r},${g},${b},0)`);
    ctx.fillStyle=grd;
    ctx.beginPath();ctx.arc(px,py,14,0,Math.PI*2);ctx.fill();

    ctx.fillStyle=`rgba(${r},${g},${b},${n.bright})`;
    ctx.beginPath();ctx.arc(px,py,n.size,0,Math.PI*2);ctx.fill();

    if(n.label){drawLabel(px,py,n.label);tickLabel(n);}
  }
}

// ── Vignette ──────────────────────────────────────────────────────────────
function drawVignette(){
  const gr=ctx.createRadialGradient(W*.7,H*.5,0,W*.7,H*.5,W*.65);
  gr.addColorStop(0,'rgba(7,11,20,0)');
  gr.addColorStop(.42,'rgba(7,11,20,0)');
  gr.addColorStop(1,'rgba(7,11,20,.92)');
  ctx.fillStyle=gr;ctx.fillRect(0,0,W,H);

  const gl=ctx.createLinearGradient(0,0,W*.44,0);
  gl.addColorStop(0,'rgba(7,11,20,.75)');
  gl.addColorStop(.35,'rgba(7,11,20,.25)');
  gl.addColorStop(1,'rgba(7,11,20,0)');
  ctx.fillStyle=gl;ctx.fillRect(0,0,W,H);
}

// ── Main loop ─────────────────────────────────────────────────────────────
let labelT=0;
function frame(){
  ctx.clearRect(0,0,W,H);

  drawGrid();
  drawEdges();
  drawAmbient();
  drawGame();
  drawVignette();

  // advance ambient
  for(const n of ants){
    n.t+=n.speed;
    if(n.t>=1){n.col=n.toCol;n.row=n.toRow;nextAnt(n);}
  }

  // advance game
  if(gState==='routing') tickPacket();
  tickParticles();

  // label spawner
  labelT++;
  if(labelT>95+Math.random()*65){labelT=0;spawnLabel();}

  requestAnimationFrame(frame);
}

// ── Input ─────────────────────────────────────────────────────────────────
canvas.addEventListener('click',e=>handleClick(e.clientX,e.clientY));
canvas.addEventListener('mousemove',e=>{
  const s=snap(e.clientX,e.clientY);
  hCol=s.col;hRow=s.row;
});
canvas.addEventListener('mouseleave',()=>{hCol=-1;hRow=-1;});

// ── Boot ──────────────────────────────────────────────────────────────────
resize();
initAmbient();
window.addEventListener('resize',()=>{resize();initAmbient();});
requestAnimationFrame(frame);

})();
