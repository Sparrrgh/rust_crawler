I created this tool solely as an exercise to familiarize myself with parallel programming in Rust.  
It simply **scans** networks searching for webservers and, when it finds one, it **screenshots** the landing page.

The final objective is to scan a /16 subnet in less than 15 minutes.

It has two modes, one uses file containing the endpoints to test (one for each line). The other mode tests an address block given the start and the end of the block and the ports to test. 
**Watch out!** the endpoints tested using the IP address block comprehend both the starting and the trailing IP. 

Gecko is required for the tool to function. You can download the latest version [here](https://github.com/mozilla/geckodriver/releases).
```
Usage:  ./crawler endpoints_file output_directory
		./crawler start_address_block end_address_block port1,port2,...,portn output_directory
```
Example:  
`./crawler 192.168.0.1 192.168.254.254 my_local_network`