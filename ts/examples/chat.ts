/**
 * Chat Conversation Example
 * 
 * This example demonstrates how to use the Soradyne protocol to create and
 * share a chat conversation between multiple peers.
 */

import { IdentityManager, ChatSDO } from '../src/index';

// Create identities for three peers
const identityManager = new IdentityManager();
const aliceId = identityManager.createIdentity('Alice');
const bobId = identityManager.createIdentity('Bob');
const charlieId = identityManager.createIdentity('Charlie');

console.log(`Created identities: Alice (${aliceId}), Bob (${bobId}), Charlie (${charlieId})`);

// Create a chat conversation
const chatConversation = new ChatSDO('Group Chat', aliceId);

// Add participants
(async () => {
  try {
    // Subscribe to chat updates
    const subscriptionId = chatConversation.subscribe(() => {
      const conversation = chatConversation.getConversation();
      
      // Get the latest message
      if (conversation.messages.length > 0) {
        const latestMessage = conversation.messages[conversation.messages.length - 1];
        console.log(`New message from ${latestMessage.senderId}: ${latestMessage.content}`);
      }
    });
    
    console.log('Starting chat conversation...');
    
    // Alice adds Bob and Charlie to the conversation
    console.log('Alice adds Bob and Charlie to the conversation...');
    await chatConversation.addMessage(aliceId, 'Hello everyone!');
    
    // Simulate Bob sending a message
    setTimeout(async () => {
      console.log('Bob sends a message...');
      await chatConversation.addMessage(bobId, 'Hi Alice! How are you?');
    }, 1000);
    
    // Simulate Charlie sending a message
    setTimeout(async () => {
      console.log('Charlie sends a message...');
      await chatConversation.addMessage(charlieId, 'Hello Alice and Bob!');
    }, 2000);
    
    // Simulate Alice responding
    setTimeout(async () => {
      console.log('Alice responds...');
      await chatConversation.addMessage(aliceId, "I'm doing great! How about you two?");
    }, 3000);
    
    // Simulate Bob responding
    setTimeout(async () => {
      console.log('Bob responds...');
      await chatConversation.addMessage(bobId, "I'm good too! This Soradyne protocol is really cool.");
    }, 4000);
    
    // Simulate Charlie responding
    setTimeout(async () => {
      console.log('Charlie responds...');
      await chatConversation.addMessage(charlieId, "Agreed! I love how it protects our data.");
    }, 5000);
    
    // End the example
    setTimeout(() => {
      console.log('Chat conversation complete!');
      chatConversation.unsubscribe(subscriptionId);
      
      // Display the final conversation
      const finalConversation = chatConversation.getConversation();
      console.log('\nFinal conversation:');
      finalConversation.messages.forEach((message) => {
        let sender = 'Unknown';
        if (message.senderId === aliceId) sender = 'Alice';
        else if (message.senderId === bobId) sender = 'Bob';
        else if (message.senderId === charlieId) sender = 'Charlie';
        
        console.log(`${sender}: ${message.content}`);
      });
    }, 6000);
  } catch (error) {
    console.error('Error in chat example:', error);
  }
})();
