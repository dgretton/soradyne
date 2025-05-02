/**
 * Heart Rate Monitor Example
 * 
 * This example demonstrates how to use the Soradyne protocol to create and
 * share heart rate data between two peers.
 */

import { IdentityManager, HeartRateSDO } from '../src/index';

// Create identities for two peers
const identityManager = new IdentityManager();
const aliceId = identityManager.createIdentity('Alice');
const bobId = identityManager.createIdentity('Bob');

console.log(`Created identities: Alice (${aliceId}), Bob (${bobId})`);

// Create a heart rate SDO for Alice
const aliceHeartRate = new HeartRateSDO('Alice Heart Rate', aliceId);

// Bob subscribes to Alice's heart rate updates
const subscriptionId = aliceHeartRate.subscribe(() => {
  const heartRate = aliceHeartRate.getHeartRate();
  console.log(`Alice's heart rate updated: ${heartRate.bpm} BPM, ${heartRate.hrv} ms HRV`);
});

console.log(`Bob subscribed to Alice's heart rate (${subscriptionId})`);

// Simulate heart rate updates from Alice
let bpm = 70;
let hrv = 45;

function simulateHeartRateUpdate() {
  // Simulate heart rate changes
  bpm += Math.floor(Math.random() * 3) - 1; // -1, 0, or +1
  hrv += Math.floor(Math.random() * 5) - 2; // -2, -1, 0, +1, or +2
  
  // Keep values in reasonable ranges
  bpm = Math.max(60, Math.min(100, bpm));
  hrv = Math.max(30, Math.min(70, hrv));
  
  // Update Alice's heart rate
  aliceHeartRate.updateHeartRate(aliceId, bpm, hrv);
  
  console.log(`Alice updated her heart rate: ${bpm} BPM, ${hrv} ms HRV`);
}

// Simulate heart rate updates every second for 10 seconds
let count = 0;
const interval = setInterval(() => {
  simulateHeartRateUpdate();
  count++;
  
  if (count >= 10) {
    clearInterval(interval);
    
    // Bob unsubscribes from Alice's heart rate updates
    aliceHeartRate.unsubscribe(subscriptionId);
    console.log(`Bob unsubscribed from Alice's heart rate`);
    
    console.log('Example complete!');
  }
}, 1000);
