all: vert frag

vert:
	glslangValidator -v src/shaders/shader.vert & mv vert.spv src/shaders/vert.spv

frag:
	glslangValidator -v src/shaders/shader.frag && mv frag.spv src/shaders/frag.spv
