CXX ?= g++
CXXFLAGS ?= -O2 -Wall -Wno-unused-variable -Wno-narrowing -I.

mc2-genlevel: main.cpp vendor/Terrain.cpp vendor/Unk_D47E0.cpp vendor/Unk_D4A30.cpp Terrain.h
	$(CXX) $(CXXFLAGS) -o $@ main.cpp vendor/Terrain.cpp vendor/Unk_D47E0.cpp vendor/Unk_D4A30.cpp

clean:
	rm -f mc2-genlevel

.PHONY: clean
