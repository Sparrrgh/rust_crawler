This is a Rust "crawler" used solely as an exercise to familiarize myself with Rust.

The final objective is to scan a /16 subnet in less than 15 minutes searching for webservers and screenshot whenever I encounter one.

It has two modes, one is using a file with the endpoints to test (One for each line). The other is two test an IP address block.  
**Watch out!** the endpoints tested using the IP address block comprehend both the starting and the trailing IP.  
```
Usage:  ./crawler endpoints_file output_directory
	./crawler start_address_block end_address_block output_directory
```
