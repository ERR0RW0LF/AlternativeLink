from socket import *


serverPort = 1337

# create UDP socket
serverSocket = socket(AF_INET, SOCK_DGRAM)

# bind socket to local port number 12000
serverSocket.bind(('', serverPort))


print("The server is ready to receive")

# loop forever
while True:
    # Read from UDP socket into message, getting client's address (client IP and port)
    message, clientAddress = serverSocket.recvfrom(2048)
    modifiedMessage = message.decode().upper()
    
    # send upper case string back to this client
    serverSocket.sendto(modifiedMessage.encode(),clientAddress)

