# include Python’s socket library
from socket import *

# Specifying server details
serverName = 'localhost'
serverPort = 1337

# create UDP socket for server
clientSocket = socket(AF_INET,SOCK_DGRAM)

# get user keyboard input
message = input('Input lowercase sentence:')

# attach server name, port to message; send into socket
clientSocket.sendto(message.encode(),(serverName, serverPort))

# read reply characters from socket into string
modifiedMessage, serverAddress = clientSocket.recvfrom(2048)

# print out received string and close socket
print(modifiedMessage.decode())
clientSocket.close()

print("Closed connection, exiting ...")
exit(1)