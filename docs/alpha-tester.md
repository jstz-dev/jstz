---
title: Jstz alpha tester guide
---

Welcome to Jstz, and thanks for being an alpha tester! Jstz combines the ease of use and flexibility of JavaScript/TypeScript with the security and transparency of Web3.
It allows you to deploy JavaScript applications called _smart functions_ in a way similar to serverless applications, where you don’t have to worry about how or where they are hosted.
Running on Jstz provides smart functions with many of the same advantages as smart contracts on Tezos and other Web3 platforms:

- Smart functions are persistent, transparent, and immutable, which allows users to trust that they will stay available and will not change how they behave or be shut down
- Smart functions are censorship-resistant, because they are deployed on distributed Jstz Smart Rollup nodes and therefore no one can block calls to them
- Smart functions have no long-term hosting cost; they incur a cost only when called

This document is a starting point for developers who want to give Jstz a try and adapt some of their web development skills to Jstz and see how it fits their needs.

## Getting help

If you have questions while you're working with Jstz, contact us on the `#jstz` channel on the Tezos Discord:

https://discord.gg/tezos

## Providing feedback

Your feedback is critical in helping us assess how well Jstz works.
We're interested in all kinds of feedback, including:

- Were you able to set up Jstz and run basic smart functions on it?
- Is the architecture and operation of Jstz clear?
- What problems did you have working with Jstz?
- Were you able to do the alpha tester challenges below?
- Is the documentation easy to understand?
- What information is missing from the docs?

Please report on these items or any other feedback you may have on this form:

https://tt-tezos.typeform.com/jstz-early-test

## Getting started

The first step to working with Jstz is to run through the [Quick start](https://jstz.tezos.com/quick_start), which covers:

- Installing Jstz
- Deploying a simple smart function
- Installing and using the Jstz development wallet
- Calling the smart function from a web application

## Development challenges

Now that you’ve gone through the Quick start, have set up your development environment, and have the basics of Jstz in hand, it’s time to put it to use with your web development skills!
Here are some challenges that can help you explore Jstz and see what web development is like with it.

Each challenge has a score based on its complexity.
How many points can you earn?

### To-do list app (2 points)

A simple starter web application is a to-do list app that lets users add to-do items, see their current task list, and mark tasks as completed.
Try this with Jstz by storing tasks and their status in the key-value store and letting users access only their own tasks.

<img src="/img/Todolist.png" alt="A list of tasks and checkboxes next to them" style={{width: 200}} />

For bonus points, add features:

- Have the user fund an account and make the smart function pay the user a tez for each complete task (1 point)
- Create a group-based system that lets managers assign tasks to users (2 points)
- Create a system for grouping and prioritizing tasks (2 points)

### Tic-tac-toe (3 points)

Jstz can provide a transparent backend for games, which allows players to trust that the game will play fair.
To demonstrate a simple game backend, write a tic-tac-toe game that stores the state of the game board and allows two players to take turns making their moves.

<img src="/img/TicTacToe.png" alt="A wireframe of a simple tic-tac-toe application" style={{width: 200}} />

**Backend**: A Jstz smart function that stores the state of the 3x3 game board, the addresses of the players, and whose turn it is.
When a player makes a move, check if they have won the game, and if so, reset the board and update a status message with the name or address of the winning player.

**Frontend**: A web application using any platform that shows the state of the board and which player’s turn it is.
It should allow the player to make a move and update the state of the board and whether a player has won.
The frontend should also allow players to start a new game.

For bonus points, expand the simple tic-tac-toe application in one or more of these ways:

- Take a payment of 1 tez from each player and send the tez to the winner (1 point)
- Allow multiple games to go on at the same time (3 points)

### Social media app (5 points)

Write a simple social media application that allows users to post messages and follow other users.
Implement a "like" feature that requires from the "liker" a symbolic amount to reward the author (e.g., 0.01 tez).

So posting is free, and is rewarded through likes.

<img src="/img/SocialMedia.png" alt="A few posts and a text box to add a new post to a social media stream" style={{width: 300}} />

For bonus points, add features:

- Provide the ability to mint a post as an NFT (5 points).
  In this case, revenues due to the original author go instead to the current owner of the NFT.
- Add group features (3 points)

**Backend**: The backend must keep track of user messages and who each user is following, and must correctly distribute revenues from likes.

**Frontend**: The frontend must allow users to post messages.
It should show separate feeds for followed users and for all users so users can find new people to follow.

### Text adventure (10 points)

Some of the best games don’t require a complex user interface, like the classic text-adventure games Zork and The Hitchhiker’s Guide to the Galaxy.
You can write a flowchart of the adventure, find data for one of these games online to use, or have an LLM generate one for you.

<img src="/img/TextAdventure.png" alt="A simple interface for the text game Zork" style={{width: 400}} />

Require 1 tez from the player for each adventure, and reward each completed adventure with 2 tez.

**Backend**: The backend must keep track of where the user is in the adventure and information about their inventory and status.
It should support multiple users who can be at any different place in the adventure at any time.

**Frontend**: The frontend needs a simple sign-in system to allow users to sign in and return to continue their progress.
It should show the previous messages from the game and allow the user to perform an action or use an item at each step in the adventure.

For bonus points, add features:

- A public leaderboard of players’ progress in the game and who has completed it in the shortest time (3 points)

### Auction site (10 points)

Jstz can handle secure payments with the tez cryptocurrency.
Create an auction site that lets users offer virtual items for sale and bid on them.

<img src="/img/AuctionSite.png" alt="Three items with their current bid and a button to bid higher" style={{width: 400}} />

**Backend**: The backend smart function must store information about the items on auction (including the seller’s address, the description of the item, and the minimum bid) and the pending bids.

**Frontend**: Using any web development platform, make an auction site that has these features:

- Lets users see the items on auction
- Lets users bid on items by sending the bid in tez
- Lets sellers put items up for auction
- Lets sellers close the auction, send the item to the winner, and return the tez from the losing bids to the owners
- Tip: To prevent losing bids from being locked in the smart function forever, allow any user to make the call to end the auction after the time for the auction expires
