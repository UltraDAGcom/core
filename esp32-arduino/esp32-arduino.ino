#include <WiFi.h>
#include <WebServer.h>
#include <ArduinoJson.h>
#include <HTTPClient.h>

// WiFi Configuration
const char* ssid = "YOUR_WIFI_SSID";
const char* password = "YOUR_WIFI_PASSWORD";

// UltraDAG Network Configuration
const char* ultradag_node = "ultradag-node-1.fly.dev";
const int ultradag_port = 8080;

WebServer server(80);
HTTPClient http;

// UltraDAG Client State
struct UltraDAGClient {
  String peer_id;
  int connected_peers;
  unsigned long latest_round;
  bool is_connected;
  int pending_txs;
} udag_client;

void setup() {
  Serial.begin(115200);
  delay(1000);
  
  Serial.println("UltraDAG ESP32 Client Starting...");
  
  // Initialize WiFi
  WiFi.begin(ssid, password);
  while (WiFi.status() != WL_CONNECTED) {
    delay(500);
    Serial.print(".");
  }
  
  Serial.println("");
  Serial.println("WiFi connected!");
  Serial.print("IP address: ");
  Serial.println(WiFi.localIP());
  
  // Initialize UltraDAG client
  udag_client.peer_id = "esp32_" + WiFi.macAddress();
  udag_client.connected_peers = 0;
  udag_client.latest_round = 0;
  udag_client.is_connected = false;
  udag_client.pending_txs = 0;
  
  // Setup HTTP server
  server.on("/", handleStatus);
  server.on("/status", handleStatus);
  server.on("/tx", handleTransaction);
  server.on("/peers", handlePeers);
  
  server.begin();
  Serial.println("HTTP server started");
  
  // Connect to UltraDAG network
  connectToUltraDAG();
}

void loop() {
  server.handleClient();
  
  // Maintain UltraDAG connection
  if (!udag_client.is_connected) {
    connectToUltraDAG();
  }
  
  // Process pending transactions
  if (udag_client.pending_txs > 0 && millis() % 5000 < 100) {
    processPendingTransactions();
  }
  
  delay(100);
}

void connectToUltraDAG() {
  Serial.println("Connecting to UltraDAG network...");
  
  // Simple connection simulation
  http.begin(String("http://") + ultradag_node + "/status");
  int httpCode = http.GET();
  
  if (httpCode == 200) {
    String payload = http.getString();
    Serial.println("Connected to UltraDAG network");
    udag_client.is_connected = true;
    udag_client.connected_peers = 1;
  } else {
    Serial.println("Failed to connect to UltraDAG network");
    udag_client.is_connected = false;
  }
  
  http.end();
}

void handleStatus() {
  String json = getStatusJson();
  server.send(200, "application/json", json);
}

void handleTransaction() {
  if (server.hasArg("plain")) {
    String txData = server.arg("plain");
    Serial.println("Received transaction: " + txData);
    
    // Parse simple transaction format: from:to:amount
    int firstColon = txData.indexOf(':');
    int secondColon = txData.indexOf(':', firstColon + 1);
    
    if (firstColon > 0 && secondColon > firstColon) {
      String from = txData.substring(0, firstColon);
      String to = txData.substring(firstColon + 1, secondColon);
      String amount = txData.substring(secondColon + 1);
      
      // Create transaction hash
      String txHash = createTxHash(from, to, amount);
      
      // Add to pending queue
      udag_client.pending_txs++;
      
      String response = "{\"hash\":\"" + txHash + "\",\"status\":\"pending\"}";
      server.send(200, "application/json", response);
      
      Serial.println("Transaction queued: " + txHash);
    } else {
      server.send(400, "application/json", "{\"error\":\"Invalid format. Use: from:to:amount\"}");
    }
  } else {
    server.send(400, "application/json", "{\"error\":\"No transaction data\"}");
  }
}

void handlePeers() {
  String json = "{\"connected_peers\":" + String(udag_client.connected_peers) + 
               ",\"known_peers\":1,\"network\":\"ultradag\"}";
  server.send(200, "application/json", json);
}

String getStatusJson() {
  String json = "{";
  json += "\"peer_id\":\"" + udag_client.peer_id + "\",";
  json += "\"connected_peers\":" + String(udag_client.connected_peers) + ",";
  json += "\"latest_round\":" + String(udag_client.latest_round) + ",";
  json += "\"status\":\"" + String(udag_client.is_connected ? "connected" : "connecting") + "\",";
  json += "\"pending_txs\":" + String(udag_client.pending_txs);
  json += "}";
  return json;
}

String createTxHash(String from, String to, String amount) {
  // Simple hash simulation (in real implementation, use BLAKE3)
  String hash = "";
  for (int i = 0; i < from.length(); i++) {
    hash += String(from.charAt(i), HEX);
  }
  for (int i = 0; i < to.length(); i++) {
    hash += String(to.charAt(i), HEX);
  }
  for (int i = 0; i < amount.length(); i++) {
    hash += String(amount.charAt(i), HEX);
  }
  return hash.substring(0, 64); // Truncate to 64 chars
}

void processPendingTransactions() {
  if (udag_client.pending_txs > 0) {
    Serial.println("Processing pending transactions...");
    // Simulate transaction processing
    udag_client.pending_txs = max(0, udag_client.pending_txs - 1);
    udag_client.latest_round++;
  }
}
