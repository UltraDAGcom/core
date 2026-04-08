// UltraDAG 3D Network Globe
// Visualizes validators and transactions in real-time on an interactive 3D globe

class NetworkGlobe {
  constructor(containerId) {
    this.container = document.getElementById(containerId);
    this.scene = null;
    this.camera = null;
    this.renderer = null;
    this.globe = null;
    this.validators = [];
    this.particles = [];
    this.animationId = null;
    
    this.init();
  }

  init() {
    this.setupScene();
    this.createGlobe();
    this.createStars();
    this.setupLights();
    this.setupControls();
    this.animate();
    this.handleResize();
    
    // Fetch and display real network data
    this.loadValidators();
    this.startTransactionAnimation();
  }

  setupScene() {
    // Scene
    this.scene = new THREE.Scene();
    this.scene.fog = new THREE.Fog(0x0a0e1a, 1, 400);

    // Camera
    const aspect = this.container.clientWidth / this.container.clientHeight;
    this.camera = new THREE.PerspectiveCamera(45, aspect, 0.1, 1000);
    this.camera.position.z = 250;

    // Renderer
    this.renderer = new THREE.WebGLRenderer({ 
      antialias: true, 
      alpha: true 
    });
    this.renderer.setSize(this.container.clientWidth, this.container.clientHeight);
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.container.appendChild(this.renderer.domElement);
  }

  createGlobe() {
    // Earth sphere
    const geometry = new THREE.SphereGeometry(100, 64, 64);
    
    // Custom shader material for glowing effect
    const material = new THREE.ShaderMaterial({
      uniforms: {
        time: { value: 0 }
      },
      vertexShader: `
        varying vec3 vNormal;
        varying vec3 vPosition;
        void main() {
          vNormal = normalize(normalMatrix * normal);
          vPosition = position;
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        }
      `,
      fragmentShader: `
        uniform float time;
        varying vec3 vNormal;
        varying vec3 vPosition;
        
        void main() {
          // Base dark blue color
          vec3 baseColor = vec3(0.04, 0.08, 0.16);
          
          // Grid lines
          float gridX = abs(fract(vPosition.x * 0.05) - 0.5) * 2.0;
          float gridY = abs(fract(vPosition.y * 0.05) - 0.5) * 2.0;
          float gridZ = abs(fract(vPosition.z * 0.05) - 0.5) * 2.0;
          float grid = min(min(gridX, gridY), gridZ);
          grid = smoothstep(0.9, 1.0, grid);
          
          // Accent color for grid
          vec3 gridColor = vec3(0.15, 0.35, 0.55) * grid * 0.3;
          
          // Fresnel glow
          float fresnel = pow(1.0 - abs(dot(vNormal, vec3(0.0, 0.0, 1.0))), 2.0);
          vec3 glowColor = vec3(0.24, 0.61, 0.91) * fresnel * 0.4;
          
          vec3 finalColor = baseColor + gridColor + glowColor;
          gl_FragColor = vec4(finalColor, 0.95);
        }
      `,
      transparent: true,
      side: THREE.DoubleSide
    });

    this.globe = new THREE.Mesh(geometry, material);
    this.scene.add(this.globe);

    // Atmosphere glow
    const atmosphereGeometry = new THREE.SphereGeometry(102, 64, 64);
    const atmosphereMaterial = new THREE.ShaderMaterial({
      vertexShader: `
        varying vec3 vNormal;
        void main() {
          vNormal = normalize(normalMatrix * normal);
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        }
      `,
      fragmentShader: `
        varying vec3 vNormal;
        void main() {
          float intensity = pow(0.6 - dot(vNormal, vec3(0.0, 0.0, 1.0)), 2.0);
          gl_FragColor = vec4(0.24, 0.61, 0.91, 1.0) * intensity;
        }
      `,
      blending: THREE.AdditiveBlending,
      side: THREE.BackSide,
      transparent: true
    });
    const atmosphere = new THREE.Mesh(atmosphereGeometry, atmosphereMaterial);
    this.scene.add(atmosphere);
  }

  createStars() {
    const starsGeometry = new THREE.BufferGeometry();
    const starsMaterial = new THREE.PointsMaterial({
      color: 0xffffff,
      size: 0.7,
      transparent: true,
      opacity: 0.8
    });

    const starsVertices = [];
    for (let i = 0; i < 2000; i++) {
      const x = (Math.random() - 0.5) * 800;
      const y = (Math.random() - 0.5) * 800;
      const z = (Math.random() - 0.5) * 800;
      starsVertices.push(x, y, z);
    }

    starsGeometry.setAttribute('position', new THREE.Float32BufferAttribute(starsVertices, 3));
    const stars = new THREE.Points(starsGeometry, starsMaterial);
    this.scene.add(stars);
  }

  setupLights() {
    const ambientLight = new THREE.AmbientLight(0xffffff, 0.5);
    this.scene.add(ambientLight);

    const pointLight = new THREE.PointLight(0x3d9be9, 1, 300);
    pointLight.position.set(50, 50, 150);
    this.scene.add(pointLight);
  }

  setupControls() {
    let isDragging = false;
    let previousMousePosition = { x: 0, y: 0 };
    let rotation = { x: 0, y: 0 };

    this.container.addEventListener('mousedown', (e) => {
      isDragging = true;
      previousMousePosition = { x: e.clientX, y: e.clientY };
    });

    this.container.addEventListener('mousemove', (e) => {
      if (isDragging) {
        const deltaX = e.clientX - previousMousePosition.x;
        const deltaY = e.clientY - previousMousePosition.y;
        
        rotation.y += deltaX * 0.005;
        rotation.x += deltaY * 0.005;
        
        previousMousePosition = { x: e.clientX, y: e.clientY };
      }
    });

    this.container.addEventListener('mouseup', () => {
      isDragging = false;
    });

    this.container.addEventListener('mouseleave', () => {
      isDragging = false;
    });

    // Store rotation for animation
    this.rotation = rotation;
  }

  async loadValidators() {
    try {
      // Fetch real network data
      const [peersRes, statusRes] = await Promise.all([
        fetch('https://ultradag-node-1.fly.dev/peers'),
        fetch('https://ultradag-node-1.fly.dev/status')
      ]);
      
      const peersData = await peersRes.json();
      const statusData = await statusRes.json();
      
      console.log('Network status:', statusData);
      console.log('Connected peers:', peersData.connected);
      
      // Use bootstrap nodes as they have real IPs we can geolocate
      if (peersData.bootstrap_nodes && peersData.bootstrap_nodes.length > 0) {
        await this.addRealNodes(peersData.bootstrap_nodes, statusData);
      } else {
        console.error('No network nodes available');
      }
    } catch (error) {
      console.error('Failed to load network data:', error);
    }
  }

  async addRealNodes(bootstrapNodes, statusData) {
    // Known locations for Fly.io regions (your bootstrap nodes are likely in these regions)
    const flyRegions = {
      '206.51.242.223': { lat: 40.7128, lon: -74.0060, name: 'New York (EWR)', region: 'ewr' },
      '137.66.57.226': { lat: 51.5074, lon: -0.1278, name: 'London (LHR)', region: 'lhr' },
      '169.155.54.169': { lat: 35.6762, lon: 139.6503, name: 'Tokyo (NRT)', region: 'nrt' },
      '169.155.55.151': { lat: -33.8688, lon: 151.2093, name: 'Sydney (SYD)', region: 'syd' }
    };

    // Add bootstrap nodes
    bootstrapNodes.forEach(node => {
      const ip = node.addr.split(':')[0];
      const location = flyRegions[ip];
      
      if (location) {
        const coords = this.latLonToVector3(location.lat, location.lon, 100);
        this.createValidatorMarker(coords.x, coords.y, coords.z, {
          name: location.name,
          ip: node.addr,
          connected: node.connected,
          type: 'bootstrap'
        });
      }
    });

    console.log(`Added ${bootstrapNodes.length} real network nodes`);
  }

  latLonToVector3(lat, lon, radius) {
    const phi = (90 - lat) * (Math.PI / 180);
    const theta = (lon + 180) * (Math.PI / 180);

    const x = -(radius * Math.sin(phi) * Math.cos(theta));
    const z = (radius * Math.sin(phi) * Math.sin(theta));
    const y = (radius * Math.cos(phi));

    return { x, y, z };
  }

  createValidatorMarker(x, y, z, validator) {
    // Glowing marker
    const geometry = new THREE.SphereGeometry(1.5, 16, 16);
    const material = new THREE.MeshBasicMaterial({
      color: 0x3d9be9,
      transparent: true,
      opacity: 0.9
    });
    const marker = new THREE.Mesh(geometry, material);
    marker.position.set(x, y, z);
    
    // Add glow
    const glowGeometry = new THREE.SphereGeometry(2.5, 16, 16);
    const glowMaterial = new THREE.MeshBasicMaterial({
      color: 0x3d9be9,
      transparent: true,
      opacity: 0.3
    });
    const glow = new THREE.Mesh(glowGeometry, glowMaterial);
    glow.position.set(x, y, z);
    
    this.scene.add(marker);
    this.scene.add(glow);
    
    this.validators.push({ marker, glow, validator, position: { x, y, z } });
  }

  startTransactionAnimation() {
    // Poll for real transactions from the network
    let lastRound = 0;
    
    const checkForTransactions = async () => {
      try {
        const response = await fetch('https://ultradag-node-1.fly.dev/status');
        const data = await response.json();
        
        // If round increased, show transaction activity
        if (data.dag_round > lastRound && this.validators.length >= 2) {
          lastRound = data.dag_round;
          
          // Create transaction particles between random nodes
          const count = Math.min(3, this.validators.length);
          for (let i = 0; i < count; i++) {
            const from = this.validators[Math.floor(Math.random() * this.validators.length)];
            const to = this.validators[Math.floor(Math.random() * this.validators.length)];
            
            if (from !== to) {
              setTimeout(() => {
                this.createTransactionParticle(from.position, to.position);
              }, i * 200);
            }
          }
        }
      } catch (error) {
        console.error('Failed to fetch transaction data:', error);
      }
    };
    
    // Check for new transactions every 2 seconds
    setInterval(checkForTransactions, 2000);
    checkForTransactions();
  }

  createTransactionParticle(start, end) {
    const geometry = new THREE.SphereGeometry(0.5, 8, 8);
    const material = new THREE.MeshBasicMaterial({
      color: 0x22d3a0,
      transparent: true,
      opacity: 1
    });
    const particle = new THREE.Mesh(geometry, material);
    particle.position.set(start.x, start.y, start.z);
    
    this.scene.add(particle);
    
    this.particles.push({
      mesh: particle,
      start: { ...start },
      end: { ...end },
      progress: 0,
      speed: 0.01 + Math.random() * 0.01
    });
  }

  animate() {
    this.animationId = requestAnimationFrame(() => this.animate());

    // Rotate globe slowly
    if (!this.rotation || (this.rotation.x === 0 && this.rotation.y === 0)) {
      this.globe.rotation.y += 0.001;
    } else {
      this.globe.rotation.x = this.rotation.x;
      this.globe.rotation.y = this.rotation.y;
    }

    // Animate validator glows
    const time = Date.now() * 0.001;
    this.validators.forEach((v, i) => {
      const scale = 1 + Math.sin(time * 2 + i) * 0.2;
      v.glow.scale.set(scale, scale, scale);
    });

    // Animate transaction particles
    this.particles = this.particles.filter(p => {
      p.progress += p.speed;
      
      if (p.progress >= 1) {
        this.scene.remove(p.mesh);
        return false;
      }
      
      // Bezier curve for arc
      const t = p.progress;
      const height = 30;
      
      const x = p.start.x + (p.end.x - p.start.x) * t;
      const y = p.start.y + (p.end.y - p.start.y) * t + Math.sin(t * Math.PI) * height;
      const z = p.start.z + (p.end.z - p.start.z) * t;
      
      p.mesh.position.set(x, y, z);
      p.mesh.material.opacity = 1 - t;
      
      return true;
    });

    this.renderer.render(this.scene, this.camera);
  }

  handleResize() {
    window.addEventListener('resize', () => {
      const width = this.container.clientWidth;
      const height = this.container.clientHeight;
      
      this.camera.aspect = width / height;
      this.camera.updateProjectionMatrix();
      this.renderer.setSize(width, height);
    });
  }

  destroy() {
    if (this.animationId) {
      cancelAnimationFrame(this.animationId);
    }
    this.container.removeChild(this.renderer.domElement);
  }
}

// Initialize when DOM is ready
if (typeof window !== 'undefined') {
  window.NetworkGlobe = NetworkGlobe;
}
