import socket
from pprint import pprint
import asyncio

global other_link_ip, finished, direct_working

finished = False
other_link_ip = None
direct_working = False

hostname = socket.gethostname()
IPAddr_list = socket.gethostbyname_ex(hostname)[2]



if len(IPAddr_list) > 1:
    for i in range(len(IPAddr_list)):
        print(f"{i}: \t{IPAddr_list[i]}")
    choice_num = input("Pleas chose the number for the correct ip address: ")
    if not(choice_num.isdigit()):
        print("Just chose a number.")
    choice_num = int(choice_num)
    IPAddr = IPAddr_list[choice_num]
elif len(IPAddr_list) == 1:
    IPAddr = IPAddr_list[0]
else:
    print("Some thing didn't work when trying to get your IPAddr.")
    raise


code = "1-3-3-7"

BROADCAST_PORT = 1337
MESSAGE = f"{code} {IPAddr}\n".encode("utf-8")

pprint(MESSAGE)


async def first_stage_discovery_broadcast():
    global finished
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.bind((IPAddr, 0))
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)

        i = 0
        while not(finished):
            sock.sendto(MESSAGE, ('255.255.255.255', BROADCAST_PORT))
            print(f"Sent broadcast #{i}")
            i += 1
            await asyncio.sleep(2)
        print("finished")
        sock.close()
    return

async def listener():
    global other_link_ip, finished, direct_working
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as listener:
        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        listener.bind(('', BROADCAST_PORT))
        listener.setblocking(False)
        while not(finished):
            try:
                message, clientAddress = listener.recvfrom(2048)
            except BlockingIOError:
                await asyncio.sleep(0.1)
                continue
            decoded_message: str = message.decode("utf-8")
            if decoded_message.startswith(f"{code} ") and decoded_message.endswith("\n"):
                possible_ip = decoded_message.removeprefix(f"{code} ").removesuffix("\n")
                if possible_ip != IPAddr and other_link_ip is None:
                    other_link_ip = possible_ip
            elif decoded_message.startswith("PING ") and decoded_message.endswith("\n"):
                print("Received a ping")
                remote_ip = decoded_message.removeprefix(f"PING ").removesuffix("\n")
                message = f"ACK {IPAddr}\n".encode("utf-8")
                listener.sendto(message, (remote_ip, BROADCAST_PORT))
            elif decoded_message.startswith("ACK ") and decoded_message.endswith("\n"):
                print("Received a ACK")
                direct_working = True
    return


async def second_stage_direct_handshake_sender():
    global other_link_ip, finished, direct_working
    with socket.socket(socket.AF_INET,socket.SOCK_DGRAM) as handshake_client:
        while not(finished or direct_working):
            print("sending ping")
            message = f"PING {IPAddr}\n".encode("utf-8")
            handshake_client.sendto(message, (other_link_ip, BROADCAST_PORT))
            await asyncio.sleep(7)
        print("Is finished")
    return


async def timeout():
    global finished
    await asyncio.sleep(60)
    finished = True
    return



async def main():
    global other_link_ip, finished
    
    # Start broadcast
    first_stage_broadcast_task = asyncio.create_task(first_stage_discovery_broadcast())
    
    
    # Start listener for first stage
    listener_task = asyncio.create_task(listener())
    
    while other_link_ip is None:
        await asyncio.sleep(0.1)
    
    print(f"other_link_ip: {other_link_ip}")
    await asyncio.to_thread(input, "Start sending when enter is pressed")
    print("Starting PING")
    
    #timeout_task = asyncio.create_task(timeout())
    
    second_stage_handshake_task = asyncio.create_task(second_stage_direct_handshake_sender())
    
    
    while not(finished):
        if direct_working:
            print("Direct connection worked")
            choice = await asyncio.to_thread(input,"Do you want to stop execution of the program? [y] ")
            if choice.lower() == 'y':
                finished = True
        await asyncio.sleep(0.1)
    
    await asyncio.gather(first_stage_broadcast_task, listener_task, second_stage_handshake_task)

if __name__ == '__main__':
    asyncio.run(main())